use std::fs;
use std::path::Path;
use audiotags::Tag;

fn decode_base64(s: &str) -> Vec<u8> {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bytes = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0;
    
    for c in s.chars() {
        if c == '=' { break; }
        if let Some(pos) = CHARSET.iter().position(|&x| x == c as u8) {
            buffer = (buffer << 6) | pos as u32;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                bytes.push((buffer >> bits) as u8);
            }
        }
    }
    bytes
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let demo_dir = Path::new("demo-media");
    if !demo_dir.exists() {
        fs::create_dir_all(demo_dir)?;
    }

    // A minimal valid silent MP3 file in base64
    let silent_mp3_base64 = "SUQzBAAAAAAAI1RTU0UAAAAPAAADTGF2ZjU2LjM2LjEwMAAAAAAAAAAAAAAA//OEAAAAAAAAAAAAAAAAAAAAAAAASW5mbwAAAA8AAAAEAAABIADAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV6urq6urq6urq6urq6urq6urq6urq6urq6v////////////////////////////////8AAAAATGF2YzU2LjQxAAAAAAAAAAAAAAAAJAAAAAAAAAAAASDs90hvAAAAAAAAAAAAAAAAAAAA//MUZAAAAAGkAAAAAAAAA0gAAAAATEFN//MUZAMAAAGkAAAAAAAAA0gAAAAARTMu//MUZAYAAAGkAAAAAAAAA0gAAAAAOTku//MUZAkAAAGkAAAAAAAAA0gAAAAANVVV";
    let mp3_bytes = decode_base64(silent_mp3_base64);

    // 1. Create dummy files
    let file1_path = demo_dir.join("AC-DC - Back In Black.mp3");
    let file2_path = demo_dir.join("02 - Metallica - Enter Sandman.mp3");
    let file3_path = demo_dir.join("03 - Pink Floyd - Time.mp3");
    let file4_path = demo_dir.join("Led Zeppelin - Stairway to Heaven.mp3"); // For filename parsing fallback (no tags)

    // Write valid base silent MP3 bytes
    fs::write(&file1_path, &mp3_bytes)?;
    fs::write(&file2_path, &mp3_bytes)?;
    fs::write(&file3_path, &mp3_bytes)?;
    fs::write(&file4_path, &mp3_bytes)?;

    println!("Created base MP3 files, now writing ID3 tags...");

    // 2. Write ID3 tags using audiotags
    // AC/DC - Back in Black (tests slash in artist name)
    if let Ok(mut tag) = Tag::new().read_from_path(&file1_path) {
        tag.set_title("Back In Black");
        tag.set_artist("AC/DC");
        tag.set_album_title("Back In Black");
        tag.set_genre("Rock");
        tag.set_year(1980);
        tag.set_track_number(1);
        tag.write_to_path(&file1_path.to_string_lossy())?;
        println!("Wrote tags to file1: AC/DC - Back In Black");
    } else {
        println!("Warning: Failed to initialize tag writer for file1");
    }

    // Metallica - Enter Sandman
    if let Ok(mut tag) = Tag::new().read_from_path(&file2_path) {
        tag.set_title("Enter Sandman");
        tag.set_artist("Metallica");
        tag.set_album_title("Metallica");
        tag.set_genre("Metal");
        tag.set_year(1991);
        tag.set_track_number(2);
        tag.write_to_path(&file2_path.to_string_lossy())?;
        println!("Wrote tags to file2: Metallica - Enter Sandman");
    } else {
        println!("Warning: Failed to initialize tag writer for file2");
    }

    // Pink Floyd - Time
    if let Ok(mut tag) = Tag::new().read_from_path(&file3_path) {
        tag.set_title("Time");
        tag.set_artist("Pink Floyd");
        tag.set_album_title("Dark Side of the Moon");
        tag.set_genre("Progressive Rock");
        tag.set_year(1973);
        tag.set_track_number(3);
        tag.write_to_path(&file3_path.to_string_lossy())?;
        println!("Wrote tags to file3: Pink Floyd - Time");
    } else {
        println!("Warning: Failed to initialize tag writer for file3");
    }

    // 3. Create relative M3U playlist
    let m3u_content = r#"#EXTM3U
#EXTINF:250,AC/DC - Back In Black
AC-DC - Back In Black.mp3
#EXTINF:331,Metallica - Enter Sandman
02 - Metallica - Enter Sandman.mp3
#EXTINF:421,Led Zeppelin - Stairway to Heaven
Led Zeppelin - Stairway to Heaven.mp3
"#;
    fs::write(demo_dir.join("favorites.m3u"), m3u_content)?;
    println!("Created favorites.m3u playlist");

    // 4. Create relative PLS playlist
    let pls_content = r#"[playlist]
NumberOfEntries=2
File1=AC-DC - Back In Black.mp3
Title1=AC/DC - Back In Black
Length1=250
File2=03 - Pink Floyd - Time.mp3
Title2=Pink Floyd - Time
Length2=421
Version=2
"#;
    fs::write(demo_dir.join("rock.pls"), pls_content)?;
    println!("Created rock.pls playlist");

    println!("Demo media generation complete!");
    Ok(())
}
