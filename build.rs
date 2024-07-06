fn main() -> Result<(), Box<dyn std::error::Error>>
{
    if let Err(x) = tonic_build::configure()
        .build_server(true)
        .out_dir("proto")
        .compile(&["proto/hello.proto", "proto/calculator.proto"], &["proto"])
    {
        eprintln!("Failed to compile proto files: {:?}. Using precompiled version.", x);
    }

    Ok(())
}
