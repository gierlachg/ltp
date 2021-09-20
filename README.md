# LTP (Line Transfer Protocol)

Serves individual lines from a static text file to clients over the network. The client-server protocol for this system
is the following:

* `GET <n>` => If `n` is a valid line number for the text file, `'OK\r\n'` is returned followed by the `n`th line from
  the text file. If `n` is NOT a valid line number, `'ERR\r\n'` is returned. Note that the lines in the file are indexed
  starting from 1, not 0.
* `QUIT` => This command disconnects the client.
* `SHUTDOWN` => This command shuts down the server.

The assumption is made that every line is newline (`'\n'`) terminated and that every character in the file is valid
ASCII.

----

Written in Rust. Using [Tokio](https://github.com/tokio-rs/tokio) framework and `async`/`await`. Each and every client
connection is handled by separate task. To speed up requests, prior to accepting connections, the file is indexed (the
size of the index is limited to ~64 MB). Lines are not cached in memory.

The performance of the system is inversely proportional to the number of lines in the file (the more lines the more
sparse the index become, the more disk reads on average have to be issued before given line is found).

As client connections are handled in a non-blocking way, using limited number of threads, I would expect the server to
scale quite well with the increase of requests/s. The performance should not deteriorate too quickly. One point might be
a bottleneck though, namely reading the lines themselves from disk. `Tokio` file support is NOT non-blocking, hence to
work with files one have to submit file requests to separate blocking thread pool. However, that might change in the
near future as `Tokio` is getting `io_uring` support.

* [Rust Docs](https://doc.rust-lang.org/std/)
* [Tokio Docs](https://docs.rs/tokio/1.11.0/tokio/)

All the dependencies are listed in `Cargo.toml`.

I have spent ~10h on it so far - still working on my Rust skills.
Didn't have time to write tests, benchmark or profile... :/

----

## Usage

_ltp -p &lt;file path&gt;_
