//! Convertion of DICOM objects into tokens.
use crate::mem::{InMemDicomObject};
use dicom_core::value::{PrimitiveValue};
use dicom_core::{DataElement, Length};
use dicom_parser::dataset::{DataToken, IntoTokens};
use std::collections::VecDeque;

/// A stream of tokens from a DICOM object.
pub struct InMemObjectTokens<E> {
    /// iterators of tokens in order of priority.
    tokens_pending: VecDeque<DataToken>,
    /// the iterator of data elements in order.
    elem_iter: E,
    /// whenever a primitive data element is yet to be processed
    elem_pending: Option<PrimitiveValue>,
    /// whether the tokens are done
    fused: bool,
}

impl<E> InMemObjectTokens<E>
where
    E: Iterator,
{
    pub fn new<T>(obj: T) -> Self
    where
        T: IntoIterator<IntoIter = E, Item = E::Item>,
    {
        InMemObjectTokens {
            tokens_pending: Default::default(),
            elem_iter: obj.into_iter(),
            elem_pending: None,
            fused: false,
        }
    }
}

impl<E> InMemObjectTokens<E>
where
    E: Iterator,
    E::Item: IntoTokens,
{
    fn next_token(&mut self) -> Option<DataToken> {
        if let Some(token) = self.tokens_pending.pop_front() {
            return Some(token);
        }

        // otherwise, expand next element, recurse
        if let Some(elem) = self.elem_iter.next() {
            // TODO eventually optimize this to be less eager
            self.tokens_pending = elem.into_tokens().collect();
            
            self.next_token()
        } else {
            // no more elements
            None
        }
    }
}

impl<E, I> Iterator for InMemObjectTokens<E>
where
    E: Iterator<Item = DataElement<I>>,
    E::Item: IntoTokens,
{
    type Item = DataToken;

    fn next(&mut self) -> Option<Self::Item> {
        if self.fused {
            return None;
        }        
        // if a data element is pending, return a value token
        if let Some(val) = self.elem_pending.take() {
            return Some(DataToken::PrimitiveValue(val));
        }

        // otherwise, consume pending tokens
        if let Some(token) = self.next_token() {
            return Some(token);
        };

        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // make a slightly better estimation for the minimum
        // number of tokens that follow: 2 tokens per element left
        (self.elem_iter.size_hint().0 * 2, None)
    }
}

impl<D> IntoTokens for InMemDicomObject<D> {
    type Iter =
        InMemObjectTokens<<InMemDicomObject<D> as IntoIterator>::IntoIter>;

    fn into_tokens(self) -> Self::Iter {
        InMemObjectTokens::new(self)
    }
}

