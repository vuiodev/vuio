use xaac_rs::{DecodeStatus, Decoder, DecoderConfig, Encoder, EncoderConfig, OutputFormat};

#[test]
fn decoder_initializes_with_default_config() {
    let decoder = Decoder::new(DecoderConfig::default()).expect("decoder should initialize");
    assert!(decoder.input_capacity() > 0);
}

#[test]
fn encodes_aac_lc_adts_frame() {
    let mut encoder = Encoder::new(EncoderConfig {
        output_format: OutputFormat::Adts,
        ..EncoderConfig::default()
    })
    .expect("encoder should initialize");

    let pcm = vec![0i16; encoder.input_frame_bytes() / 2];
    let encoded = encoder
        .encode_i16_interleaved(&pcm)
        .expect("frame should encode");

    assert!(!encoded.data.is_empty());
}

#[test]
fn encodes_partial_frame_with_zero_padding() {
    let mut encoder = Encoder::new(EncoderConfig {
        output_format: OutputFormat::Raw,
        ..EncoderConfig::default()
    })
    .expect("encoder should initialize");

    let partial = vec![0u8; encoder.input_frame_bytes() / 2];
    let encoded = encoder
        .encode_pcm_bytes_with_padding(&partial)
        .expect("partial frame should encode");

    assert_eq!(encoded.padded_input_bytes, partial.len());
    assert!(!encoder.audio_specific_config().is_empty());
    assert!(!encoded.packet.data.is_empty());
}

#[test]
fn decodes_adts_stream_in_small_chunks() {
    let mut encoder = Encoder::new(EncoderConfig {
        output_format: OutputFormat::Adts,
        ..EncoderConfig::default()
    })
    .expect("encoder should initialize");

    let mut bitstream = Vec::new();
    for frame_index in 0..4 {
        let pcm = vec![frame_index as i16; encoder.input_frame_bytes() / 2];
        let packet = encoder
            .encode_i16_interleaved(&pcm)
            .expect("frame should encode");
        bitstream.extend_from_slice(&packet.data);
    }

    let mut decoder = Decoder::new(DecoderConfig::default()).expect("decoder should initialize");
    let mut decoded_frames = 0usize;
    let mut offset = 0usize;
    while offset < bitstream.len() {
        let end = (offset + 37).min(bitstream.len());
        match decoder
            .decode_stream_chunk(&bitstream[offset..end])
            .expect("decode chunk should not fail")
        {
            DecodeStatus::Frame(frame) => {
                decoded_frames += 1;
                assert!(!frame.pcm.is_empty());
                assert!(frame.stream_info.channels >= 1);
            }
            DecodeStatus::NeedMoreInput(_) => {}
            DecodeStatus::EndOfStream => break,
        }
        offset = end;
    }

    loop {
        match decoder.finish().expect("finish should succeed") {
            DecodeStatus::Frame(frame) => {
                decoded_frames += 1;
                assert!(!frame.pcm.is_empty());
            }
            DecodeStatus::NeedMoreInput(_) => continue,
            DecodeStatus::EndOfStream => break,
        }
    }

    assert!(decoded_frames > 0);
    assert!(decoder.stream_info().is_some());
}
