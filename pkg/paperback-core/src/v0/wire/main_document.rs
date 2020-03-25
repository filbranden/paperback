/*
 * paperback: paper backup generator suitable for long-term storage
 * Copyright (C) 2018-2020 Aleksa Sarai <cyphar@cyphar.com>
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

use crate::v0::{
    wire::{prefixes::*, FromWire, ToWire},
    ChaChaPolyNonce, Identity, MainDocument, MainDocumentBuilder, MainDocumentMeta,
    CHACHAPOLY_NONCE_LENGTH,
};

use unsigned_varint::encode;

// Internal only -- users can't see MainDocumentMeta.
#[doc(hidden)]
impl ToWire for MainDocumentMeta {
    fn to_wire(&self) -> Vec<u8> {
        let mut buffer = encode::u32_buffer();
        let mut bytes = vec![];

        // Encode version.
        encode::u32(self.version, &mut buffer)
            .iter()
            .for_each(|b| bytes.push(*b));

        // Encode quorum size.
        encode::u32(self.quorum_size, &mut buffer)
            .iter()
            .for_each(|b| bytes.push(*b));

        bytes
    }
}

// Internal only -- users can't see MainDocumentMeta.
#[doc(hidden)]
impl FromWire for MainDocumentMeta {
    fn from_wire_partial(input: &[u8]) -> Result<(Self, &[u8]), String> {
        use crate::v0::wire::nom_helpers;
        use nom::{combinator::complete, IResult};

        fn parse(input: &[u8]) -> IResult<&[u8], MainDocumentMeta> {
            let (input, version) = nom_helpers::u32()(input)?;
            let (input, quorum_size) = nom_helpers::u32()(input)?;

            let meta = MainDocumentMeta {
                version,
                quorum_size,
            };

            Ok((input, meta))
        }
        let parse = complete(parse);

        let (remain, meta) = parse(input).map_err(|err| format!("{:?}", err))?;
        Ok((meta, remain))
    }
}

// Internal only -- users can't see MainDocumentBuilder.
#[doc(hidden)]
impl ToWire for MainDocumentBuilder {
    fn to_wire(&self) -> Vec<u8> {
        let mut buffer = encode::u64_buffer();
        let mut bytes = vec![];

        // Encode metadata.
        bytes.append(&mut self.meta.to_wire());

        // Encode nonce.
        encode::u64(PREFIX_CHACHA20POLY1305_NONCE, &mut buffer)
            .iter()
            .chain(&self.nonce)
            .for_each(|b| bytes.push(*b));

        // Encode ciphertext.
        encode::u64(PREFIX_CHACHA20POLY1305_CIPHERTEXT, &mut buffer)
            .iter()
            .chain(encode::usize(
                self.ciphertext.len(),
                &mut encode::usize_buffer(),
            ))
            .chain(&self.ciphertext)
            .for_each(|b| bytes.push(*b));

        bytes
    }
}

// Internal only -- users can't see MainDocumentBuilder.
#[doc(hidden)]
impl FromWire for MainDocumentBuilder {
    fn from_wire_partial(input: &[u8]) -> Result<(Self, &[u8]), String> {
        use crate::v0::wire::nom_helpers;
        use nom::{bytes::complete::take, combinator::complete, IResult};

        fn parse(input: &[u8]) -> IResult<&[u8], (ChaChaPolyNonce, &[u8])> {
            let (input, _) = nom_helpers::u64_tag(PREFIX_CHACHA20POLY1305_NONCE)(input)?;
            let (input, nonce) = take(CHACHAPOLY_NONCE_LENGTH)(input)?;

            let nonce = {
                let mut buffer = ChaChaPolyNonce::default();
                buffer.copy_from_slice(nonce);
                buffer
            };

            let (input, _) = nom_helpers::u64_tag(PREFIX_CHACHA20POLY1305_CIPHERTEXT)(input)?;
            let (input, length) = nom_helpers::usize()(input)?;
            let (input, ciphertext) = take(length)(input)?;

            Ok((input, (nonce, ciphertext)))
        }
        let parse = complete(parse);

        let (meta, input) = MainDocumentMeta::from_wire_partial(input)?;
        let (remain, (nonce, ciphertext)) = parse(input).map_err(|err| format!("{:?}", err))?;

        Ok((
            MainDocumentBuilder {
                meta,
                nonce,
                ciphertext: ciphertext.into(),
            },
            remain,
        ))
    }
}

impl ToWire for MainDocument {
    fn to_wire(&self) -> Vec<u8> {
        let mut bytes = vec![];

        bytes.append(&mut self.inner.to_wire());
        bytes.append(&mut self.identity.to_wire());

        bytes
    }
}

impl FromWire for MainDocument {
    fn from_wire_partial(input: &[u8]) -> Result<(Self, &[u8]), String> {
        let (inner, input) = MainDocumentBuilder::from_wire_partial(input)?;
        let (identity, input) = Identity::from_wire_partial(input)?;

        if inner.meta.version != 0 {
            return Err(format!(
                "main document version must be '0' not '{}'",
                inner.meta.version
            ));
        }

        Ok((MainDocument { inner, identity }, input))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[quickcheck]
    fn main_document_roundtrip(main: MainDocument) {
        let main2 = MainDocument::from_wire(main.to_wire()).unwrap();
        assert_eq!(main, main2);

        let inner2 = MainDocumentBuilder::from_wire(main.inner.to_wire()).unwrap();
        assert_eq!(main.inner, inner2);

        let meta2 = MainDocumentMeta::from_wire(main.inner.meta.to_wire()).unwrap();
        assert_eq!(main.inner.meta, meta2);
    }
}