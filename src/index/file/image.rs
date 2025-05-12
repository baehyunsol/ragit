use super::{AtomicToken, FileReaderImpl};
use crate::error::Error;
use crate::index::BuildConfig;
use crate::uid::Uid;
use ragit_fs::{extension, read_bytes};
use ragit_pdl::ImageType;
use resvg::render;
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{self, Tree};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::io::Cursor;

/*
TODO: There's a subtle problem with ragit's image reader.

1. All the images have to go through `normalize_image` before `generate_chunk`.
  - In order to create an AtomicToken, an image needs a uid.
  - A uid is generated from a hash value of the bytes of the image, and `normalize_image` alters the bytes.
2. Each file reader implements `load_tokens`, which creates AtomicTokens.
3. That means each file reader is responsible for calling `normalize_image`. If it forgets to do so, it will break the knowledge-base.
4. If you're adding a new file reader, you're very likely to forget this fact.
*/

pub type Path = String;

#[derive(Clone, PartialEq)]
pub struct Image {
    pub uid: Uid,
    pub image_type: ImageType,
    pub bytes: Vec<u8>,
}

impl fmt::Debug for Image {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        fmt.debug_struct("Image")
            .field("uid", &self.uid)
            .field("image_type", &self.image_type)
            .finish()
    }
}

pub fn normalize_image(bytes: Vec<u8>, image_type: ImageType) -> Result<Vec<u8>, Error> {
    let mut dynamic_image = match image_type {
        ImageType::Svg => {
            let bytes = render_svg_to_png(&bytes)?;
            image::load_from_memory_with_format(
                &bytes,
                ImageType::Png.try_into()?,
            )?
        },
        _ => image::load_from_memory_with_format(
            &bytes,
            image_type.try_into()?,
        )?,
    };

    if dynamic_image.width() > 1024 || dynamic_image.height() > 1024 {
        dynamic_image = dynamic_image.resize(1024, 1024, image::imageops::FilterType::Triangle);
    }

    // no modification at all
    else if image_type == ImageType::Png {
        return Ok(bytes);
    }

    let result = vec![];
    let mut writer = Cursor::new(result);
    dynamic_image.write_to(&mut writer, image::ImageFormat::Png)?;
    let result = writer.into_inner();

    Ok(result)
}

pub struct ImageReader {
    path: Path,
    tokens: Vec<AtomicToken>,
    image_type: ImageType,
    strict_mode: bool,
    is_exhausted: bool,
}

impl FileReaderImpl for ImageReader {
    fn new(path: &str, config: &BuildConfig) -> Result<Self, Error> {
        Ok(ImageReader {
            path: path.to_string(),
            tokens: vec![],
            image_type: ImageType::from_extension(&extension(path)?.unwrap_or(String::new()))?,
            strict_mode: config.strict_file_reader,
            is_exhausted: false,
        })
    }

    fn load_tokens(&mut self) -> Result<(), Error> {
        if self.is_exhausted {
            Ok(())
        }

        else {
            let bytes = read_bytes(&self.path)?;

            match normalize_image(bytes.clone(), self.image_type) {
                Ok(bytes) => {
                    let uid = Uid::new_image(&bytes);
                    self.tokens.push(AtomicToken::Image(Image {
                        bytes,
                        image_type: ImageType::Png,
                        uid,
                    }));
                    self.is_exhausted = true;
                    Ok(())
                },
                Err(e) => if self.strict_mode {
                    Err(e)
                } else {
                    // TODO: split `s` if it's too long
                    let s = String::from_utf8_lossy(&bytes).to_string();
                    self.tokens.push(AtomicToken::String {
                        data: s.clone(),
                        char_len: s.chars().count(),
                    });
                    self.is_exhausted = true;
                    Ok(())
                },
            }
        }
    }

    fn pop_all_tokens(&mut self) -> Result<Vec<AtomicToken>, Error> {
        let mut result = vec![];
        std::mem::swap(&mut self.tokens, &mut result);
        Ok(result)
    }

    fn has_more_to_read(&self) -> bool {
        !self.is_exhausted
    }

    fn key(&self) -> String {
        String::from("image_reader_v0")
    }
}

#[derive(Default, Deserialize, Serialize)]
pub struct ImageDescription {
    pub extracted_text: String,
    pub explanation: String,
}

fn render_svg_to_png(svg: &[u8]) -> Result<Vec<u8>, Error> {
    let tree_options = usvg::Options {
        // It returns `None` if `width` or `height` is negative.
        // So we can safely unwrap the result.
        default_size: usvg::Size::from_wh(1024.0, 1024.0).unwrap(),
        ..usvg::Options::default()
    };
    let tree = Tree::from_data(svg, &tree_options)?;
    let svg_size = tree.size();

    // As far as I know, it returns None only if size is 0
    let mut pixmap = Pixmap::new(
        svg_size.width() as u32,
        svg_size.height() as u32,
    ).unwrap_or_else(
        || Pixmap::new(1024, 1024).unwrap()
    );
    render(
        &tree,
        Transform::identity(),
        &mut pixmap.as_mut(),
    );

    Ok(pixmap.encode_png()?)
}
