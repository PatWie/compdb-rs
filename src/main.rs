use anyhow::{Context, Result};
use clap::Parser;
use crossbeam_deque::{Injector, Steal};
use dashmap::DashMap;
use regex::Regex;
use rustc_hash::FxHasher;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::hash::BuildHasherDefault;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::LazyLock;

// Fast hasher type aliases
type FxDashMap<K, V> = DashMap<K, V, BuildHasherDefault<FxHasher>>;
type FxHashSet<T> = HashSet<T, BuildHasherDefault<FxHasher>>;

// Directory ID mapping for ultra-fast cache keys
static DIR_TO_ID: LazyLock<FxDashMap<PathBuf, u32>> = LazyLock::new(FxDashMap::default);
static NEXT_DIR_ID: AtomicU32 = AtomicU32::new(0);

fn get_dir_id(path: &Path) -> u32 {
    let path_buf = path.to_path_buf();
    if let Some(id) = DIR_TO_ID.get(&path_buf) {
        *id
    } else {
        let id = NEXT_DIR_ID.fetch_add(1, Ordering::Relaxed);
        DIR_TO_ID.insert(path_buf, id);
        id
    }
}

type ResolveCacheKey = (String, u32); // (include, dir_id)

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    name = "compdb",
    about = "Compilation database manipulation tool",
    long_about = "A fast tool for manipulating compilation databases (compile_commands.json)"
)]
struct Cli {
    #[arg(
        short = 'p',
        long = "build-path",
        help = "Build directory path(s) containing compile_commands.json"
    )]
    build_paths: Vec<PathBuf>,

    #[arg(
        value_enum,
        default_value = "list",
        help = "Command to execute"
    )]
    command: Command,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum Command {
    List,
    Version,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompileCommand {
    directory: String,
    file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    arguments: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output: Option<String>,
}



static INCLUDE_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\s*#\s*include\s+[<"]([^>"]+)[>"]"#).unwrap());

fn extract_includes(content: &str) -> Vec<String> {
    INCLUDE_PATTERN
        .captures_iter(content)
        .map(|cap| cap[1].to_string())
        .collect()
}

fn find_header_files(
    compile_commands: &[CompileCommand],
) -> Result<Vec<CompileCommand>> {
    // Build per-file include maps
    let file_to_includes: FxDashMap<PathBuf, Vec<PathBuf>> = FxDashMap::default();
    let file_to_command: FxDashMap<PathBuf, CompileCommand> = FxDashMap::default();
    let mut all_system_dirs_set: FxHashSet<PathBuf> = FxHashSet::default();

    for cmd in compile_commands {
        let file_path = PathBuf::from(&cmd.file);
        let (project_dirs, system_dirs) = extract_include_directories_for_command(cmd);
        all_system_dirs_set.extend(system_dirs.clone());
        let all_dirs: Vec<PathBuf> = project_dirs.into_iter().chain(system_dirs.into_iter()).collect();
        file_to_includes.insert(file_path.clone(), all_dirs);
        file_to_command.insert(file_path, cmd.clone());
    }
    let all_system_dirs: Vec<PathBuf> = all_system_dirs_set.into_iter().collect();

    let processed_headers: FxDashMap<String, PathBuf> = FxDashMap::default(); // header -> source that found it
    let resolve_cache: FxDashMap<ResolveCacheKey, Option<String>> = FxDashMap::default();
    let exists_cache: FxDashMap<PathBuf, bool> = FxDashMap::default();

    // Work item is now (file_to_process, context_source_file)
    let work_queue = Injector::<(PathBuf, PathBuf)>::new();

    // Seed the queue with ONLY valid C/C++ source files
    for cmd in compile_commands {
        let path = PathBuf::from(&cmd.file);
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            if matches!(ext, "c" | "cpp" | "cc" | "cxx") && path.exists() {
                // For a source file, its context is itself
                work_queue.push((path.clone(), path));
            }
        }
    }

    rayon::scope(|s| {
        for _ in 0..rayon::current_num_threads() {
            s.spawn(|_| {
                let mut local_work: Vec<(PathBuf, PathBuf)> = Vec::with_capacity(64);

                loop {
                    if local_work.is_empty() {
                        match work_queue.steal() {
                            Steal::Success(item) => local_work.push(item),
                            Steal::Retry => continue,
                            Steal::Empty => break,
                        }
                    }

                    while let Some((file_path, context_path)) = local_work.pop() {
                        let is_source = match file_path.extension().and_then(|s| s.to_str()) {
                            Some(ext) => matches!(ext, "c" | "cpp" | "cc" | "cxx" | "h" | "hpp" | "hh" | "hxx"),
                            None => true,
                        };
                        if !is_source { continue; }

                        // Get the correct include paths using the context
                        if let Some(include_dirs) = file_to_includes.get(&context_path) {
                            if let Ok(content) = fs::read_to_string(&file_path) {
                                let includes = extract_includes(&content);
                                for include in includes {
                                    if let Some(header_path_str) = resolve_header_path(&include, &include_dirs, &file_path, &resolve_cache, &exists_cache) {
                                        if processed_headers.insert(header_path_str.clone(), context_path.clone()).is_none() {
                                            if !is_system_header(&header_path_str, &all_system_dirs) {
                                                let header_path = PathBuf::from(header_path_str);
                                                local_work.push((header_path, context_path.clone()));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            });
        }
    });

    let header_commands: Vec<CompileCommand> = processed_headers
        .into_iter()
        .filter_map(|(header_path, source_path)| {
            if let Some(source_cmd_ref) = file_to_command.get(&source_path) {
                let source_cmd = source_cmd_ref.value();
                return Some(CompileCommand {
                    directory: source_cmd.directory.clone(),
                    file: header_path,
                    command: source_cmd.command.clone(),
                    arguments: source_cmd.arguments.clone(),
                    output: None,
                });
            }
            None
        })
        .collect();

    Ok(header_commands)
}

fn is_system_header(header_path: &str, system_dirs: &[PathBuf]) -> bool {
    let path = Path::new(header_path);
    system_dirs.iter().any(|sys_dir| path.starts_with(sys_dir))
}

fn extract_include_directories_for_command(cmd: &CompileCommand) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut project_dirs = FxHashSet::default();
    let mut system_dirs = FxHashSet::default();

    let args = if let Some(ref args) = cmd.arguments {
        args.clone()
    } else if let Some(ref command) = cmd.command {
        command.split_whitespace().map(std::string::ToString::to_string).collect()
    } else {
        return (Vec::new(), Vec::new());
    };

    let mut i = 0;
    while i < args.len() {
        if args[i] == "-I" && i + 1 < args.len() {
            let path = PathBuf::from(&args[i + 1]);
            if is_system_path(&path) {
                system_dirs.insert(path);
            } else {
                project_dirs.insert(path);
            }
            i += 2;
        } else if args[i].starts_with("-I") {
            let path_str = &args[i][2..];
            if !path_str.is_empty() {
                let path = PathBuf::from(path_str);
                if is_system_path(&path) {
                    system_dirs.insert(path);
                } else {
                    project_dirs.insert(path);
                }
            }
            i += 1;
        } else if args[i] == "-isystem" && i + 1 < args.len() {
            system_dirs.insert(PathBuf::from(&args[i + 1]));
            i += 2;
        } else {
            i += 1;
        }
    }

    (project_dirs.into_iter().collect(), system_dirs.into_iter().collect())
}


fn is_system_path(path: &Path) -> bool {
    let path_str = path.to_string_lossy();
    path_str.starts_with("/usr/") ||
    path_str.starts_with("/opt/") ||
    path_str.contains("toolchain") ||
    path_str.contains("sysroot") ||
    path_str.contains("gcc") ||
    path_str.contains("clang")
}

fn batch_check_exists(paths: &[PathBuf], exists_cache: &FxDashMap<PathBuf, bool>) -> Vec<bool> {
    paths.iter().map(|path| {
        *exists_cache.entry(path.clone()).or_insert_with(|| path.exists())
    }).collect()
}

fn resolve_header_path(
    include: &str,
    include_dirs: &[PathBuf],
    source_file: &Path,
    cache: &FxDashMap<ResolveCacheKey, Option<String>>,
    exists_cache: &FxDashMap<PathBuf, bool>,
) -> Option<String> {
    let source_dir = source_file.parent().unwrap_or_else(|| Path::new(""));

    // Get ultra-fast directory ID instead of hashing full PathBuf
    let dir_id = get_dir_id(source_dir);
    let key = (include.to_string(), dir_id);

    if let Some(cached) = cache.get(&key) {
        return cached.value().clone();
    }

    // Build candidate paths
    let relative_path = source_dir.join(include);
    let mut candidate_paths = vec![relative_path.clone()];
    candidate_paths.extend(include_dirs.iter().map(|dir| dir.join(include)));

    // Batch check existence
    let exists_results = batch_check_exists(&candidate_paths, exists_cache);

    let result = if exists_results[0] {
        fs::canonicalize(&relative_path).ok().and_then(|p| p.to_str().map(String::from))
    } else {
        for (i, exists) in exists_results.iter().skip(1).enumerate() {
            if *exists {
                return fs::canonicalize(&candidate_paths[i + 1]).ok().and_then(|p| p.to_str().map(String::from));
            }
        }
        None
    };

    cache.insert(key, result.clone());
    result
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::List => list_command(&cli),
        Command::Version => version_command(),
    }
}

fn version_command() -> Result<()> {
    println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    Ok(())
}

fn list_command(cli: &Cli) -> Result<()> {
    let start_time = std::time::Instant::now();
    let mut all_commands = Vec::new();

    let build_paths = if cli.build_paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        cli.build_paths.clone()
    };

    for build_path in build_paths {
        let compile_commands_path = build_path.join("compile_commands.json");

        if !compile_commands_path.exists() {
            continue;
        }

        let content = fs::read_to_string(&compile_commands_path)
            .with_context(|| format!("Failed to read {}", compile_commands_path.display()))?;

        let commands: Vec<CompileCommand> = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse JSON from {}", compile_commands_path.display()))?;

        all_commands.extend(commands);
    }

    // Extract header files
    let header_commands = find_header_files(&all_commands)?;

    // Combine original commands with header commands
    all_commands.extend(header_commands);

    let output = serde_json::to_string_pretty(&all_commands)?;
    println!("{output}");

    let elapsed = start_time.elapsed();
    eprintln!("Generated {} compile commands in {:.3}s", all_commands.len(), elapsed.as_secs_f64());

    Ok(())
}
