# Git HTTP Router

`git-http-router` is a tiny, high-performance async HTTP server written in Rust that natively serves local Git repositories over HTTP. It acts as a lightweight CGI wrapper around Git's built-in `git http-backend` binary, unlocking native HTTP clone and push functionality for local repositories.

## Why?

Modern GitOps tools (like Flux) heavily rely on authenticated protocols like HTTP(S) and SSH. The native `git://` protocol (used by `git daemon`) is unauthenticated and often blocked by client-side validation in tools like `flux bootstrap`. 

`git-http-router` solves this by spinning up a fast HTTP server that talks to the `git http-backend`, allowing you to bypass these restrictions and seamlessly interact with your local Git repositories natively via HTTP `git clone http://127.0.0.1:8080/my-repo.git`.

## Installation

You can install `git-http-router` directly from source via Cargo:

```bash
cargo install --path .
```

## Usage

Start the server by specifying the port and the root directory containing your Git repositories:

```bash
git-http-router --port 8080 --root /path/to/your/git/repos
```

Options:
- `--port, -p`: The port to listen on (default: 8080)
- `--root, -r`: The root directory of your Git repositories (default: `.`)

Once running, you can clone, pull, and push to your repositories just like any remote HTTP git server:
```bash
git clone http://127.0.0.1:8080/my-repo.git
```

## Integration

This tool pairs perfectly with local GitOps testing workflows (such as with [gitops-bootstrap-tui](https://github.com/basarsubasi/gitops-bootstrap-tui)), allowing Flux controllers to seamlessly synchronize with a local directory.
