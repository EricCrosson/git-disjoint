pub(crate) trait Increment: Sized {
    type Error;

    fn increment(self, count: u32) -> Result<(Self, u32), Self::Error>;
}
