pub mod trivial;
pub mod slow;
#[cfg(feature = "socat")]
pub mod socat_port;

#[cfg(test)]
mod tests {
    use super::*;

}
