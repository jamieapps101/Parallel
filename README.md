# Parallel

Jamie Apps

---

A utility for running commands in parallel from stdin.

To install

```sh
cargo install --path .
```

The following

```sh
cat list_of_commands.txt | parallel --threads 2
```
will run entries from *list_of_commands.txt* 2 at a time.

Threads can identify their own index though the use of a THREAD_INDEX environment variable injected into each process's environment.

