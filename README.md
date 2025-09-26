# compdb-rs: Faster Than a Keystroke ⚡

*If Python compdb was a snail, this is a caffeinated cheetah strapped to a tachyon-powered rocket.*

### TL;DR

Rust is love. Rust is speed. Rust is crab. 🦀
If you don’t like it, write your own `compdb` in Haskell and wait until 2047 for it to compile.

-----

## The Tragedy of Python Compdb 😢

Once upon a time, someone thought: *“You know what we need? A single-threaded Python script to crawl a multi-million-line C++ codebase.”* And it worked. Sort of. If by "worked" you mean:

  * Took longer than your CI pipeline to run a coffee errand.
  * Gave your 32-core CPU one sad little thread to chew on.
  * Got lost in your include paths and missed hundreds of files.
  * Left you wondering if you should’ve just typed everything out by hand.

**Result:** You get your compilation database… eventually. Right after you’ve learned woodworking, become a sourdough expert, and mastered the banjo.

-----

## Enter compdb-rs 🎩✨

`compdb-rs` doesn’t ask politely. It shows up in Rust, kicks down the door screaming:
**“THREADS. CONTEXT-AWARE RESOLUTION. CACHING. HYPERDRIVE.”**

It doesn’t ask the filesystem.
It **interrogates** the filesystem. With intensity. With purpose. With *Rayon*.

And it’s fast. **So fast you'll think the result is an error message.** It's not. It's just done.

-----

## Benchmarks (a.k.a. The Public Humiliation)

| Metric | Python `compdb` | `compdb-rs` (The Upgrade) | Reaction |
| :--- | :--- | :--- | :--- |
| **Time** | 12+ seconds | **89 milliseconds** | “Did it even run??” |
| **Files found** | \~2,600 (The ones it felt like finding) | **3,200+ (All of them)** | “Python was blindfolded??” |
| **Correctness**| ❌ Incorrectly resolves headers | ✅ **Context-aware resolution** | “Oh, so that’s why my IDE was broken.” |
| **CPU Cores** | 1 (The lonely one) | **ALL OF THEM** | Even the cursed one labeled `Core #0` |
| **Vibes** | Sad trombone 🎺 | **Death metal solo 🎸🔥** | 🤘 |

-----

## Installation 🛠️

```bash
# From crates.io (because you're a professional)
cargo install compdb

# From source (for those who like to watch the world compile)
git clone https://github.com/patwie/compdb-rs
cd compdb-rs
cargo build --release

# Run it like you stole it
./target/release/compdb -p /path/to/build list > compile_commands.json
```

That’s it. No obscure flags. No 12-page manual. Just pure, unadulterated speed.

-----

## Features That Slap 🎉

  * **Ludicrous Speed:** So fast it finishes before you can switch windows to check on it.
  * **Correctness by Default:** Finds more files because it *actually* uses the right include paths for each source file. Fixing bugs makes things faster, apparently.
  * **Parallel File Scanning:** Uses a *work-stealing* scheduler, which sounds aggressive because it is. Your CPU cores won't know what hit them.
  * **Intelligent Caching:** Because hitting the disk a million times is not a personality trait.
  * **System Header Pruning:** Knows when *not* to go spelunking inside `<iostream>` for the 57th time.
  * **Zero Config:** It just… works. Like Apple, but without the $999 monitor stand.

-----

## Why Rust? 🦀

  * **Python made us wait.** Like *actually wait*. Like “oh cool, I can go brew a pot of coffee” levels of waiting.
  * **C++ would’ve segfaulted out of spite.** You know it, I know it, the core dump knows it.
  * **Java would’ve required 47 XML config files** and a PhD in “AbstractSingletonProxyFactoryBean.”
  * **Node.js would’ve spawned 400MB of dependencies** just to read a file.
  * **Go would've complained about generics** until we gave up.
  * **Bash was… not an option.** Unless you enjoy crying yourself to sleep at night.

And, because we have taste.

-----

## Contributing 👐

Think you can shave off another 20ms? Please, try. We dare you.

Ways to help:

  * Make it even faster (good luck).
  * Fix bugs (if you find one, frame it).
  * Add even more sass to this README.

-----

## Disclaimer ⚠️

`compdb-rs` is not liable for:

  * Your sudden inability to tolerate any tool that takes longer than a second to run.
  * The awkward silence after you brag about your 89ms compile DB and no one understands.
  * Your manager asking why you haven’t rewritten the entire company toolchain in Rust.
  * Existential dread when you realize you spent years waiting for a Python script.
  * A sudden urge to benchmark everything in your life (toothbrushing speed, stair climbing throughput, etc).

-----

**Built with:** Rust, Rayon, DashMap, caffeine, and a burning hatred of wasted clock cycles.

**Motto**: *"Why wait when you can be done already?"*
