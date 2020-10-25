pub use std::{
    cell::{Ref, RefCell, RefMut},
    iter::FromIterator,
    marker::PhantomData,
    ops::Range,
    slice::{Iter, IterMut},
};

use crate::{
    err::SerError,
    neli_constants::MAX_NL_LENGTH,
    nl::Nlmsghdr,
    nlattr::Nlattr,
    rtnl::Rtattr,
    types::traits::{
        BufferOps, DeBufferOps, FlagBufferOps, GenlBufferOps, NlBufferOps, RtBufferOps,
        SerBufferOps, SockBufferOps,
    },
};

/// A buffer of bytes that, when used, can avoid unnecessary allocations.
#[derive(Debug, PartialEq)]
pub struct Buffer(Vec<u8>);

impl AsRef<[u8]> for Buffer {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl AsMut<[u8]> for Buffer {
    fn as_mut(&mut self) -> &mut [u8] {
        self.0.as_mut_slice()
    }
}

impl BufferOps for Buffer {
    fn new() -> Self {
        Buffer(Vec::new())
    }

    fn from_slice(slice: &[u8]) -> Self {
        Buffer(Vec::from(slice))
    }

    fn extend_from_slice(&mut self, slice: &[u8]) {
        self.0.extend_from_slice(slice)
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A buffer to serialize into
#[derive(Debug)]
pub struct SerBuffer<'a>(Vec<u8>, &'a PhantomData<()>);

impl<'a> SerBuffer<'a> {
    fn from_vec(inner: Vec<u8>) -> Self {
        SerBuffer(inner, &PhantomData)
    }
}

impl<'a> AsRef<[u8]> for SerBuffer<'a> {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl<'a> AsMut<[u8]> for SerBuffer<'a> {
    fn as_mut(&mut self) -> &mut [u8] {
        self.0.as_mut_slice()
    }
}

impl<'a> SerBufferOps<'a> for SerBuffer<'a> {
    fn new(size_hint: Option<usize>) -> Self {
        let inner = match size_hint {
            Some(sh) => vec![0; sh],
            None => vec![0; MAX_NL_LENGTH],
        };
        SerBuffer(inner, &PhantomData)
    }

    fn split(mut self, range: Range<usize>) -> (Option<Self>, Self, Option<Self>) {
        let end_buffer = self.0.split_off(range.end);
        let middle_buffer = self.0.split_off(range.start);
        (
            if self.len() == 0 { None } else { Some(self) },
            SerBuffer::from_vec(middle_buffer),
            if end_buffer.is_empty() {
                None
            } else {
                Some(SerBuffer::from_vec(end_buffer))
            },
        )
    }

    fn join(&mut self, start: Option<Self>, end: Option<Self>) -> Result<(), SerError<'a>> {
        if let Some(mut s) = start {
            s.0.extend_from_slice(self.as_ref());
            self.0 = s.0
        }
        if let Some(e) = end {
            self.0.extend(e.0.into_iter())
        }
        Ok(())
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A buffer to deserialize from
#[derive(Debug)]
pub struct DeBuffer<'a>(&'a [u8]);

impl<'a> From<&'a [u8]> for DeBuffer<'a> {
    fn from(slice: &'a [u8]) -> Self {
        DeBuffer(slice)
    }
}

impl<'a> AsRef<[u8]> for DeBuffer<'a> {
    fn as_ref(&self) -> &[u8] {
        self.0
    }
}

impl<'a> DeBufferOps<'a> for DeBuffer<'a> {
    fn slice(&self, slice: Range<usize>) -> Self {
        DeBuffer::from(&self.0[slice])
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// An immutable reference to the socket buffer.
pub struct SockBufferRef<'a>(Ref<'a, Vec<u8>>);

impl<'a> AsRef<[u8]> for SockBufferRef<'a> {
    fn as_ref(&self) -> &[u8] {
        (*self.0).as_slice()
    }
}

/// A mutable reference to the socket buffer.
pub struct SockBufferRefMut<'a>(RefMut<'a, Vec<u8>>);

impl<'a> AsMut<[u8]> for SockBufferRefMut<'a> {
    fn as_mut(&mut self) -> &mut [u8] {
        (*self.0).as_mut_slice()
    }
}

/// A buffer to hold data read from sockets
pub struct SockBuffer(pub RefCell<Vec<u8>>);

impl SockBufferOps for SockBuffer {
    fn new() -> Self {
        SockBuffer(RefCell::new(vec![0; MAX_NL_LENGTH]))
    }

    fn get_ref(&self) -> Option<SockBufferRef> {
        self.0.try_borrow().ok().map(SockBufferRef)
    }

    fn get_mut(&self) -> Option<SockBufferRefMut> {
        self.0.try_borrow_mut().ok().map(SockBufferRefMut)
    }
}

impl<'a> From<&'a [u8]> for SockBuffer {
    fn from(s: &'a [u8]) -> Self {
        SockBuffer(RefCell::new(s.to_vec()))
    }
}

/// A buffer of netlink messages.
#[derive(Debug, PartialEq)]
pub struct NlBuffer<T, P>(Vec<Nlmsghdr<T, P>>);

impl<T, P> FromIterator<Nlmsghdr<T, P>> for NlBuffer<T, P> {
    fn from_iter<I>(i: I) -> Self
    where
        I: IntoIterator<Item = Nlmsghdr<T, P>>,
    {
        NlBuffer(Vec::from_iter(i))
    }
}

impl<T, P> AsRef<[Nlmsghdr<T, P>]> for NlBuffer<T, P> {
    fn as_ref(&self) -> &[Nlmsghdr<T, P>] {
        self.0.as_slice()
    }
}

impl<'a, T, P> NlBufferOps<'a, T, P> for NlBuffer<T, P>
where
    T: 'a,
    P: 'a,
{
    type Iter = Iter<'a, Nlmsghdr<T, P>>;
    type IterMut = IterMut<'a, Nlmsghdr<T, P>>;

    fn new() -> Self {
        NlBuffer(Vec::new())
    }

    fn push(&mut self, msg: Nlmsghdr<T, P>) {
        self.0.push(msg);
    }

    fn iter(&'a self) -> Self::Iter {
        self.0.iter()
    }

    fn iter_mut(&'a mut self) -> Self::IterMut {
        self.0.iter_mut()
    }
}

impl<T, P> IntoIterator for NlBuffer<T, P> {
    type Item = Nlmsghdr<T, P>;
    type IntoIter = <Vec<Nlmsghdr<T, P>> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

/// A buffer of generic netlink attributes.
#[derive(Debug, PartialEq)]
pub struct GenlBuffer<T, P>(Vec<Nlattr<T, P>>);

impl<T, P> AsRef<[Nlattr<T, P>]> for GenlBuffer<T, P> {
    fn as_ref(&self) -> &[Nlattr<T, P>] {
        self.0.as_slice()
    }
}

impl<T, P> FromIterator<Nlattr<T, P>> for GenlBuffer<T, P> {
    fn from_iter<I>(i: I) -> Self
    where
        I: IntoIterator<Item = Nlattr<T, P>>,
    {
        GenlBuffer(Vec::from_iter(i))
    }
}

impl<T, P> IntoIterator for GenlBuffer<T, P> {
    type Item = Nlattr<T, P>;
    type IntoIter = <Vec<Nlattr<T, P>> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a, T, P> GenlBufferOps<'a, T, P> for GenlBuffer<T, P>
where
    T: 'a,
    P: 'a,
{
    type Iter = Iter<'a, Nlattr<T, P>>;
    type IterMut = IterMut<'a, Nlattr<T, P>>;

    fn new() -> Self {
        GenlBuffer(Vec::new())
    }

    fn push(&mut self, attr: Nlattr<T, P>) {
        self.0.push(attr)
    }

    fn iter(&'a self) -> Self::Iter {
        self.0.iter()
    }

    fn iter_mut(&'a mut self) -> Self::IterMut {
        self.0.iter_mut()
    }
}

/// A buffer of rtnetlink attributes.
#[derive(Debug)]
pub struct RtBuffer<T, P>(Vec<Rtattr<T, P>>);

impl<T, P> FromIterator<Rtattr<T, P>> for RtBuffer<T, P> {
    fn from_iter<I>(i: I) -> Self
    where
        I: IntoIterator<Item = Rtattr<T, P>>,
    {
        RtBuffer(Vec::from_iter(i))
    }
}

impl<T, P> IntoIterator for RtBuffer<T, P> {
    type Item = Rtattr<T, P>;
    type IntoIter = <Vec<Rtattr<T, P>> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<T, P> AsRef<[Rtattr<T, P>]> for RtBuffer<T, P> {
    fn as_ref(&self) -> &[Rtattr<T, P>] {
        self.0.as_slice()
    }
}

impl<'a, T, P> RtBufferOps<'a, T, P> for RtBuffer<T, P>
where
    T: 'a,
    P: 'a,
{
    type Iter = Iter<'a, Rtattr<T, P>>;
    type IterMut = IterMut<'a, Rtattr<T, P>>;

    fn new() -> Self {
        RtBuffer(Vec::new())
    }

    fn push(&mut self, attr: Rtattr<T, P>) {
        self.0.push(attr)
    }

    fn iter(&'a self) -> Self::Iter {
        self.0.iter()
    }

    fn iter_mut(&'a mut self) -> Self::IterMut {
        self.0.iter_mut()
    }
}

/// A buffer of flag constants.
#[derive(Debug, PartialEq)]
pub struct FlagBuffer<T>(pub Vec<T>);

impl<'a, T> From<&'a [T]> for FlagBuffer<T>
where
    T: Clone,
{
    fn from(slice: &[T]) -> Self {
        FlagBuffer(Vec::from(slice))
    }
}

impl<'a, T> FlagBufferOps<'a, T> for FlagBuffer<T>
where
    T: 'a + PartialEq + Clone,
{
    type Iter = std::slice::Iter<'a, T>;

    fn empty() -> Self {
        FlagBuffer(Vec::new())
    }

    fn contains(&self, elem: &T) -> bool {
        self.0.contains(elem)
    }

    fn set(&mut self, flag: T) {
        if !self.0.contains(&flag) {
            self.0.push(flag)
        }
    }

    fn unset(&mut self, flag: &T) {
        self.0.retain(|e| flag != e)
    }

    fn iter(&'a self) -> Self::Iter {
        self.0.iter()
    }
}