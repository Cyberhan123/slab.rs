use anyhow::anyhow;
use ffmpeg::{Rational, codec, encoder, format, media};
use ffmpeg_next as ffmpeg;

pub(crate) fn supports_output_format(format_name: &str) -> bool {
    matches!(
        format_name.to_ascii_lowercase().as_str(),
        "mp4" | "mkv" | "webm" | "mov" | "avi" | "m4v"
    )
}

pub(crate) fn remux_media(source_path: &str, output_path: &str) -> anyhow::Result<()> {
    ffmpeg::init().map_err(|error| anyhow!("failed to initialize ffmpeg-next: {error}"))?;

    let mut ictx =
        format::input(source_path).map_err(|error| anyhow!("failed to open input: {error}"))?;
    let mut octx =
        format::output(output_path).map_err(|error| anyhow!("failed to open output: {error}"))?;

    let mut stream_mapping = vec![0_i32; ictx.nb_streams() as usize];
    let mut ist_time_bases = vec![Rational(0, 1); ictx.nb_streams() as usize];
    let mut ost_index = 0_i32;

    for (ist_index, ist) in ictx.streams().enumerate() {
        let medium = ist.parameters().medium();
        if medium != media::Type::Audio && medium != media::Type::Video && medium != media::Type::Subtitle {
            stream_mapping[ist_index] = -1;
            continue;
        }

        stream_mapping[ist_index] = ost_index;
        ist_time_bases[ist_index] = ist.time_base();
        ost_index += 1;

        let mut ost = octx
            .add_stream(encoder::find(codec::Id::None))
            .map_err(|error| anyhow!("failed to add output stream: {error}"))?;
        ost.set_parameters(ist.parameters());

        // Reset codec_tag to avoid container-specific incompatibilities during remux.
        unsafe {
            (*ost.parameters().as_mut_ptr()).codec_tag = 0;
        }
    }

    octx.set_metadata(ictx.metadata().to_owned());
    octx.write_header()
        .map_err(|error| anyhow!("failed to write output header: {error}"))?;

    for (stream, mut packet) in ictx.packets() {
        let ist_index = stream.index();
        let mapped_index = stream_mapping[ist_index];
        if mapped_index < 0 {
            continue;
        }

        let ost = octx
            .stream(mapped_index as usize)
            .ok_or_else(|| anyhow!("failed to resolve output stream index {mapped_index}"))?;
        packet.rescale_ts(ist_time_bases[ist_index], ost.time_base());
        packet.set_position(-1);
        packet.set_stream(mapped_index as usize);
        packet
            .write_interleaved(&mut octx)
            .map_err(|error| anyhow!("failed to write remuxed packet: {error}"))?;
    }

    octx.write_trailer()
        .map_err(|error| anyhow!("failed to write output trailer: {error}"))?;

    Ok(())
}
