use futures_util::{Stream, TryStreamExt};
use vortex::array::chunked::ChunkedArray;
use vortex::stream::ArrayStream;
use vortex::{Array, IntoArrayData, ViewContext};
use vortex_dtype::DType;
use vortex_error::{vortex_bail, VortexResult};

use crate::io::VortexWrite;
use crate::MessageWriter;

pub struct ArrayWriter<W: VortexWrite> {
    msgs: MessageWriter<W>,
    view_ctx: ViewContext,

    view_ctx_range: Option<ByteRange>,
    array_layouts: Vec<ArrayLayout>,
}

impl<W: VortexWrite> ArrayWriter<W> {
    pub fn new(write: W, view_ctx: ViewContext) -> Self {
        Self {
            msgs: MessageWriter::new(write),
            view_ctx,
            view_ctx_range: None,
            array_layouts: vec![],
        }
    }

    pub fn view_context_range(&self) -> Option<ByteRange> {
        self.view_ctx_range
    }

    pub fn array_layouts(&self) -> &[ArrayLayout] {
        &self.array_layouts
    }

    pub fn into_inner(self) -> W {
        self.msgs.into_inner()
    }

    pub async fn write_context(mut self) -> VortexResult<Self> {
        if self.view_ctx_range.is_some() {
            vortex_bail!("View context already written");
        }

        let begin = self.msgs.tell();
        self.msgs.write_view_context(&self.view_ctx).await?;
        let end = self.msgs.tell();

        self.view_ctx_range = Some(ByteRange { begin, end });

        Ok(self)
    }

    async fn write_dtype(&mut self, dtype: &DType) -> VortexResult<ByteRange> {
        let begin = self.msgs.tell();
        self.msgs.write_dtype(dtype).await?;
        let end = self.msgs.tell();
        Ok(ByteRange { begin, end })
    }

    async fn write_array_chunks<S>(&mut self, mut stream: S) -> VortexResult<ChunkLayout>
    where
        S: Stream<Item = VortexResult<Array>> + Unpin,
    {
        let mut byte_offsets = vec![self.msgs.tell()];
        let mut row_offsets = vec![0];
        let mut row_offset = 0;

        while let Some(chunk) = stream.try_next().await? {
            row_offset += chunk.len() as u64;
            row_offsets.push(row_offset);
            self.msgs
                .write_chunk(&self.view_ctx, chunk.into_array_data())
                .await?;
            byte_offsets.push(self.msgs.tell());
        }

        Ok(ChunkLayout {
            byte_offsets,
            row_offsets,
        })
    }

    pub async fn write_array_stream<S: ArrayStream + Unpin>(
        mut self,
        mut array_stream: S,
    ) -> VortexResult<Self> {
        let dtype_pos = self.write_dtype(array_stream.dtype()).await?;
        let chunk_pos = self.write_array_chunks(&mut array_stream).await?;
        self.array_layouts.push(ArrayLayout {
            dtype: dtype_pos,
            chunks: chunk_pos,
        });
        Ok(self)
    }

    pub async fn write_array(self, array: Array) -> VortexResult<Self> {
        if let Ok(chunked) = ChunkedArray::try_from(&array) {
            self.write_array_stream(chunked.array_stream()).await
        } else {
            self.write_array_stream(array.into_array_stream()).await
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct ByteRange {
    pub begin: u64,
    pub end: u64,
}

#[derive(Clone, Debug)]
pub struct ArrayLayout {
    pub dtype: ByteRange,
    pub chunks: ChunkLayout,
}

#[derive(Clone, Debug)]
pub struct ChunkLayout {
    pub byte_offsets: Vec<u64>,
    pub row_offsets: Vec<u64>,
}
