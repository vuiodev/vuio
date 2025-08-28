use std::env;
use std::path::Path;
use std::fs;

fn main() {
    // Tell Cargo to rerun this build script if the schema changes
    println!("cargo:rerun-if-changed=schemas/media.fbs");
    
    let out_dir = env::var("OUT_DIR").unwrap();
    let schema_path = "schemas/media.fbs";
    let output_file = format!("{}/media_generated.rs", out_dir);
    
    // Generate Rust code from FlatBuffer schema
    if Path::new(schema_path).exists() {
        match flatc_rust::run(flatc_rust::Args {
            inputs: &[Path::new(schema_path)],
            out_dir: Path::new(&out_dir),
            ..Default::default()
        }) {
            Ok(_) => {
                println!("cargo:warning=FlatBuffer schema compiled successfully");
                return;
            }
            Err(e) => {
                println!("cargo:warning=Failed to compile FlatBuffer schema: {}", e);
                println!("cargo:warning=Falling back to stub implementation");
            }
        }
    } else {
        println!("cargo:warning=FlatBuffer schema file not found: {}", schema_path);
    }
    
    // Create a stub implementation if flatc is not available
    let stub_content = r#"
// Stub implementation for when flatc is not available
pub mod media_d_b {
    use flatbuffers::FlatBufferBuilder;
    
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum BatchOperationType {
        Insert = 0,
        Update = 1,
        Delete = 2,
        Upsert = 3,
    }
    
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum IndexType {
        PathIndex = 0,
        DirectoryIndex = 1,
        ArtistIndex = 2,
        AlbumIndex = 3,
        GenreIndex = 4,
    }
    
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum WALOperation {
        BatchInsert = 0,
        BatchUpdate = 1,
        BatchDelete = 2,
        IndexUpdate = 3,
        Checkpoint = 4,
    }
    
    // Stub MediaFile table
    pub struct MediaFile<'a> {
        _tab: flatbuffers::Table<'a>,
    }
    
    impl<'a> MediaFile<'a> {
        pub fn id(&self) -> u64 { 0 }
        pub fn path(&self) -> Option<&str> { None }
        pub fn canonical_path(&self) -> Option<&str> { None }
        pub fn filename(&self) -> Option<&str> { None }
        pub fn size(&self) -> u64 { 0 }
        pub fn modified(&self) -> u64 { 0 }
        pub fn mime_type(&self) -> Option<&str> { None }
        pub fn duration(&self) -> u64 { 0 }
        pub fn title(&self) -> Option<&str> { None }
        pub fn artist(&self) -> Option<&str> { None }
        pub fn album(&self) -> Option<&str> { None }
        pub fn genre(&self) -> Option<&str> { None }
        pub fn track_number(&self) -> u32 { 0 }
        pub fn year(&self) -> u32 { 0 }
        pub fn album_artist(&self) -> Option<&str> { None }
        pub fn created_at(&self) -> u64 { 0 }
        pub fn updated_at(&self) -> u64 { 0 }
        
        pub fn create<'b>(
            _builder: &mut FlatBufferBuilder<'b>,
            _args: &MediaFileArgs<'b>,
        ) -> flatbuffers::WIPOffset<MediaFile<'b>> {
            flatbuffers::WIPOffset::new(0)
        }
    }
    
    pub struct MediaFileArgs<'a> {
        pub id: u64,
        pub path: Option<flatbuffers::WIPOffset<&'a str>>,
        pub canonical_path: Option<flatbuffers::WIPOffset<&'a str>>,
        pub filename: Option<flatbuffers::WIPOffset<&'a str>>,
        pub size: u64,
        pub modified: u64,
        pub mime_type: Option<flatbuffers::WIPOffset<&'a str>>,
        pub duration: u64,
        pub title: Option<flatbuffers::WIPOffset<&'a str>>,
        pub artist: Option<flatbuffers::WIPOffset<&'a str>>,
        pub album: Option<flatbuffers::WIPOffset<&'a str>>,
        pub genre: Option<flatbuffers::WIPOffset<&'a str>>,
        pub track_number: u32,
        pub year: u32,
        pub album_artist: Option<flatbuffers::WIPOffset<&'a str>>,
        pub created_at: u64,
        pub updated_at: u64,
    }
    
    // Stub MediaFileBatch table
    pub struct MediaFileBatch<'a> {
        _tab: flatbuffers::Table<'a>,
    }
    
    impl<'a> MediaFileBatch<'a> {
        pub fn files(&self) -> Option<flatbuffers::Vector<flatbuffers::ForwardsUOffset<MediaFile>>> { None }
        pub fn batch_id(&self) -> u64 { 0 }
        pub fn timestamp(&self) -> u64 { 0 }
        pub fn operation_type(&self) -> BatchOperationType { BatchOperationType::Insert }
        
        pub fn create<'b>(
            _builder: &mut FlatBufferBuilder<'b>,
            _args: &MediaFileBatchArgs<'b>,
        ) -> flatbuffers::WIPOffset<MediaFileBatch<'b>> {
            flatbuffers::WIPOffset::new(0)
        }
    }
    
    pub struct MediaFileBatchArgs<'a> {
        pub files: Option<flatbuffers::WIPOffset<flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<MediaFile<'a>>>>>,
        pub batch_id: u64,
        pub timestamp: u64,
        pub operation_type: BatchOperationType,
    }
}
"#;
    
    // Write the stub file
    if let Err(e) = fs::write(&output_file, stub_content) {
        println!("cargo:warning=Failed to write stub file: {}", e);
    } else {
        println!("cargo:warning=Created stub FlatBuffer implementation");
    }
}