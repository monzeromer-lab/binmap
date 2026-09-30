# corpus/dotnet

Fixtures for the C# backend, in the shapes `TOOLING-DOTNET` documents.

**These were not produced by a real .NET SDK.** There is none on the machine
this was developed on, so unlike every other corpus here they have not met the
toolchain they describe. `§3` marks several property names as changing across
SDK versions, and `binmap_dotnet::configuration::UNVERIFIED` lists them.

```bash
cargo run -p binmap-eval -- dgml corpus/dotnet/example.dgml --why App.Models.Widget
cargo run -p binmap-eval -- trim-warnings corpus/dotnet/publish.log
```

The graph exists to demonstrate the one question DWARF cannot answer: nobody
asked for `Widget` to be in the binary, and the chain from `Main` through
`Serialize` to a reflection edge is why trimming could not remove it.
