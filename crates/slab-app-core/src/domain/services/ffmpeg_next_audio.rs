use anyhow::{Context, anyhow};
use ffmpeg_next as ffmpeg;
use ffmpeg::{codec, format, frame, media};

pub(crate) fn supports_output_format(format_name: &str) -> bool {
    matches!(
        format_name.to_ascii_lowercase().as_str(),
        "mp3" | "wav" | "flac" | "ogg" | "opus" | "aac" | "m4a"
    )
}

pub(crate) fn transcode_audio(source_path: &str, output_path: &str) -> anyhow::Result<()> {
    ffmpeg::init().map_err(|error| anyhow!("failed to initialize ffmpeg-next: {error}"))?;

    let mut ictx =
        format::input(source_path).map_err(|error| anyhow!("failed to open input: {error}"))?;
    let mut octx =
        format::output(output_path).map_err(|error| anyhow!("failed to open output: {error}"))?;

    let input_stream = ictx
        .streams()
        .best(media::Type::Audio)
        .ok_or_else(|| anyhow!("no audio stream found in input"))?;
    let input_stream_index = input_stream.index();

    let decoder_context = codec::context::Context::from_parameters(input_stream.parameters())
        .map_err(|error| anyhow!("failed to create decoder context: {error}"))?;
    let mut decoder = decoder_context
        .decoder()
        .audio()
        .map_err(|error| anyhow!("failed to create audio decoder: {error}"))?;
    decoder
        .set_parameters(input_stream.parameters())
        .map_err(|error| anyhow!("failed to set decoder parameters: {error}"))?;

    let encoder_codec = ffmpeg::encoder::find(octx.format().codec(output_path, media::Type::Audio))
        .ok_or_else(|| anyhow!("failed to find output audio encoder"))?
        .audio()
        .context("selected output codec is not an audio encoder")?;

    let global = octx
        .format()
        .flags()
        .contains(ffmpeg::format::flag::Flags::GLOBAL_HEADER);

    let mut output_stream = octx
        .add_stream(encoder_codec)
        .map_err(|error| anyhow!("failed to add output stream: {error}"))?;
    let encoder_context = codec::context::Context::from_parameters(output_stream.parameters())
        .map_err(|error| anyhow!("failed to create encoder context: {error}"))?;
    let mut encoder = encoder_context
        .encoder()
        .audio()
        .map_err(|error| anyhow!("failed to create audio encoder: {error}"))?;

    let channel_layout = encoder_codec
        .channel_layouts()
        .map(|layouts| layouts.best(decoder.channel_layout().channels()))
        .unwrap_or(ffmpeg::channel_layout::ChannelLayout::STEREO);

    if global {
        encoder.set_flags(ffmpeg::codec::flag::Flags::GLOBAL_HEADER);
    }

    encoder.set_rate(decoder.rate() as i32);
    encoder.set_channel_layout(channel_layout);
    encoder.set_format(
        encoder_codec
            .formats()
            .context("unknown supported output formats")?
            .next()
            .ok_or_else(|| anyhow!("output codec has no supported sample format"))?,
    );
    encoder.set_bit_rate(decoder.bit_rate());
    encoder.set_time_base((1, decoder.rate() as i32));
    output_stream.set_time_base((1, decoder.rate() as i32));

    let mut encoder = encoder
        .open_as(encoder_codec)
        .map_err(|error| anyhow!("failed to open encoder: {error}"))?;
    output_stream.set_parameters(&encoder);

    let out_time_base = output_stream.time_base();
    drop(output_stream);

    octx.set_metadata(ictx.metadata().to_owned());
    octx.write_header()
        .map_err(|error| anyhow!("failed to write output header: {error}"))?;

    let in_time_base = decoder.time_base();

    for (stream, packet) in ictx.packets() {
        if stream.index() != input_stream_index {
            continue;
        }

        let mut packet = packet;
        packet.rescale_ts(stream.time_base(), in_time_base);
        decoder
            .send_packet(&packet)
            .map_err(|error| anyhow!("failed to send packet to decoder: {error}"))?;
        drain_decoded_frames(&mut decoder, &mut encoder, &mut octx, in_time_base, out_time_base)?;
    }

    decoder
        .send_eof()
        .map_err(|error| anyhow!("failed to send decoder EOF: {error}"))?;
    drain_decoded_frames(&mut decoder, &mut encoder, &mut octx, in_time_base, out_time_base)?;

    encoder
        .send_eof()
        .map_err(|error| anyhow!("failed to send encoder EOF: {error}"))?;
    drain_encoded_packets(&mut encoder, &mut octx, in_time_base, out_time_base)?;

    octx.write_trailer()
        .map_err(|error| anyhow!("failed to write output trailer: {error}"))?;

    Ok(())
}

fn drain_decoded_frames(
    decoder: &mut codec::decoder::Audio,
    encoder: &mut codec::encoder::Audio,
    octx: &mut format::context::Output,
    in_time_base: ffmpeg::Rational,
    out_time_base: ffmpeg::Rational,
) -> anyhow::Result<()> {
    let mut decoded = frame::Audio::empty();
    while decoder.receive_frame(&mut decoded).is_ok() {
        let timestamp = decoded.timestamp();
        decoded.set_pts(timestamp);
        encoder
            .send_frame(&decoded)
            .map_err(|error| anyhow!("failed to send frame to encoder: {error}"))?;
        drain_encoded_packets(encoder, octx, in_time_base, out_time_base)?;
    }

    Ok(())
}

fn drain_encoded_packets(
    encoder: &mut codec::encoder::Audio,
    octx: &mut format::context::Output,
    in_time_base: ffmpeg::Rational,
    out_time_base: ffmpeg::Rational,
) -> anyhow::Result<()> {
    let mut encoded = ffmpeg::Packet::empty();
    while encoder.receive_packet(&mut encoded).is_ok() {
        encoded.set_stream(0);
        encoded.rescale_ts(in_time_base, out_time_base);
        encoded
            .write_interleaved(octx)
            .map_err(|error| anyhow!("failed to write encoded packet: {error}"))?;
    }

    Ok(())
}
