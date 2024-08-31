//! Conversion of DICOM objects into tokens.
use crate::mem::InMemDicomObject;
use dicom_core::DataElement;
use dicom_parser::dataset::{DataToken, IntoTokens, IntoTokensOptions};
use std::collections::VecDeque;

/// A stream of tokens from a DICOM object.
pub struct InMemObjectTokens<E> {
    /// iterators of tokens in order of priority.
    tokens_pending: VecDeque<DataToken>,
    /// the iterator of data elements in order.
    elem_iter: E,
    /// whether the tokens are done
    fused: bool,
    /// Options to take into account when generating tokens
    token_options: IntoTokensOptions,
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
            fused: false,
            token_options: Default::default(),
        }
    }

    pub fn new_with_options<T>(obj: T, token_options: IntoTokensOptions) -> Self
    where
        T: IntoIterator<IntoIter = E, Item = E::Item>,
    {
        InMemObjectTokens {
            tokens_pending: Default::default(),
            elem_iter: obj.into_iter(),
            fused: false,
            token_options,
        }
    }
}

impl<P, I, E> Iterator for InMemObjectTokens<E>
where
    E: Iterator<Item = DataElement<I, P>>,
    E::Item: IntoTokens,
{
    type Item = DataToken;

    fn next(&mut self) -> Option<Self::Item> {
        if self.fused {
            return None;
        }

        // otherwise, consume pending tokens
        if let Some(token) = self.tokens_pending.pop_front() {
            return Some(token);
        }

        // otherwise, expand next element, recurse
        if let Some(elem) = self.elem_iter.next() {
            self.tokens_pending = if self.token_options == Default::default() {
                elem.into_tokens()
            } else {
                elem.into_tokens_with_options(self.token_options)
            }
            .collect();

            self.next()
        } else {
            // no more elements
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // make a slightly better estimation for the minimum
        // number of tokens that follow: 2 tokens per element left
        (self.elem_iter.size_hint().0 * 2, None)
    }
}

impl<D> IntoTokens for InMemDicomObject<D> {
    type Iter = InMemObjectTokens<<InMemDicomObject<D> as IntoIterator>::IntoIter>;

    fn into_tokens(self) -> Self::Iter {
        InMemObjectTokens::new(self)
    }

    fn into_tokens_with_options(self, mut options: IntoTokensOptions) -> Self::Iter {
        //This is required for recursing with the correct option
        options.force_invalidate_sq_length |= self.charset_changed;
        InMemObjectTokens::new_with_options(self, options)
    }
}

impl<'a, D> IntoTokens for &'a InMemDicomObject<D>
where
    D: Clone,
{
    type Iter =
        InMemObjectTokens<std::iter::Cloned<<&'a InMemDicomObject<D> as IntoIterator>::IntoIter>>;

    fn into_tokens(self) -> Self::Iter {
        self.into_tokens_with_options(Default::default())
    }

    fn into_tokens_with_options(self, mut options: IntoTokensOptions) -> Self::Iter {
        options.force_invalidate_sq_length |= self.charset_changed;

        InMemObjectTokens::new_with_options(self.into_iter().cloned(), options)
    }
}
