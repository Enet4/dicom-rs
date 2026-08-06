//! Private module to assist in making safe memory allocations

const DANGEROUS_CAPACITY: usize = 16_777_216;

/// Simplified allocation error
#[derive(Debug)]
pub enum Error {
    /// Could not allocate via `try_reserve`
    TryReserve,
    /// Allocation would have been dangerous
    Dangerous,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Perform an allocation, safeguarded from extreme cases.
#[allow(dead_code)]
pub fn guarded_alloc(
    capacity: usize,
    encapsulated_size: usize,
    ratio_threshold: u32,
) -> Result<Vec<u8>> {
    // do not expect an allocation N times larger than the given encapsulated data size
    if capacity >= DANGEROUS_CAPACITY && encapsulated_size < capacity / ratio_threshold as usize {
        return Err(Error::Dangerous);
    }

    let mut out = Vec::new();
    out.try_reserve_exact(capacity)
        .map_err(|_| Error::TryReserve)?;
    Ok(out)
}

/// Reserve extra capacity for a vector (with zeros), safeguarded from extreme cases.
#[allow(dead_code)]
pub fn guarded_reserve(
    out: &mut Vec<u8>,
    additional_capacity: usize,
    encapsulated_size: usize,
    ratio_threshold: u32,
) -> Result<()> {
    // do not expect an allocation N times larger than the given encapsulated data size
    if additional_capacity >= DANGEROUS_CAPACITY
        && encapsulated_size < additional_capacity / ratio_threshold as usize
    {
        return Err(Error::Dangerous);
    }

    out.try_reserve_exact(additional_capacity)
        .map_err(|_| Error::TryReserve)?;
    Ok(())
}

/// Perform a resize of a vector (with zeros), safeguarded from extreme cases.
#[allow(dead_code)]
pub fn guarded_resize(
    out: &mut Vec<u8>,
    additional_capacity: usize,
    encapsulated_size: usize,
    ratio_threshold: u32,
) -> Result<()> {
    guarded_reserve(out, additional_capacity, encapsulated_size, ratio_threshold)?;
    out.resize(out.len() + additional_capacity, 0);
    Ok(())
}
