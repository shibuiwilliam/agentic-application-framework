# hello-agent

The smallest end-to-end AAF example. It seeds two read-only
capabilities (`cap-greet`, `cap-translate`), compiles a natural-language
goal into an `IntentEnvelope`, plans against the registry, executes the
graph, and prints the trace.

## Run it

```bash
cargo run -p aaf-server -- run examples/hello-agent/aaf.yaml
```

Expected output:

```text
hello-agent v0.1.0 starting
registered 2 capabilities
compiled intent int-... (AnalyticalIntent)
plan: 1 step(s)
✓ completed 1 steps
trace status = Completed, steps recorded = 1
done
```

## Try the other CLI subcommands against this config

```bash
# validate the YAML schema
cargo run -p aaf-server -- validate examples/hello-agent/aaf.yaml

# do an ad-hoc semantic discovery (uses ./aaf.yaml by default,
# so cd into the example dir first or copy aaf.yaml to the project root)
( cd examples/hello-agent && cargo run -p aaf-server -- discover translate )

# compile a goal string into a JSON envelope (no execution)
cargo run -p aaf-server -- compile "translate greetings to japanese"
```
