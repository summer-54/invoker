use {
    async_compression::tokio::{bufread::GzipDecoder, write::GzipEncoder},
    std::{io, sync::Arc},
    tokio::{
        io::{AsyncBufRead, AsyncRead, AsyncWrite, AsyncWriteExt, BufWriter},
        sync::Mutex,
    },
    tokio_tar::{Archive, Builder, Header},
};

use crate::Result;

pub async fn decompress<R: AsyncBufRead + Unpin>(data: R) -> Archive<GzipDecoder<R>> {
    Archive::new(GzipDecoder::new(data))
}

pub async fn compress<R: AsyncBufRead + Unpin>(items: &[(&str, &[u8])]) -> Result<Box<[u8]>> {
    let mut archive_builder = Builder::new(GzipEncoder::new(Vec::new()));
    for (path, item) in Box::<[(&str, &[u8])]>::from(items) {
        let mut header = Header::new_gnu();
        header.set_size(item.len() as u64);
        header.set_cksum();

        archive_builder.append_data(&mut header, path, item);
    }

    let mut archive = archive_builder.into_inner().await?;
    archive.shutdown().await?;

    Ok(archive.into_inner().into_boxed_slice())
}
