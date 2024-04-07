fn main() -> Result<(), Box<dyn std::error::Error>> {
    match tonic_build::compile_protos("proto/hello.proto") {
        Ok(_) => Ok(()),
        Err(e) => {
            eprintln!("Failed to compile proto files: {:?}", e);
            Ok(()) // Ignore missing protoc
        }
    }
}
