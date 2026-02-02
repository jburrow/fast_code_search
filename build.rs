fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::compile_protos("proto/search.proto")?;
    tonic_prost_build::compile_protos("proto/semantic_search.proto")?;
    Ok(())
}
