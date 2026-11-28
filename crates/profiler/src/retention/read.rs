//! The OS validates readable ranges. A failed or partial copy supplies no words.

pub(super) fn words(address: usize, output: &mut [usize]) -> bool {
    if output.is_empty() {
        return true;
    }
    if address.checked_add(size_of_val(output)).is_none() {
        return false;
    }
    #[cfg(windows)]
    {
        use windows::Win32::System::{
            Diagnostics::Debug::ReadProcessMemory, Threading::GetCurrentProcess,
        };
        let mut copied = 0;
        // No reference into target memory is formed. The destination is owned,
        // aligned and sized; Windows handles invalid or unmapped target ranges.
        unsafe {
            ReadProcessMemory(
                GetCurrentProcess(),
                address as *const _,
                output.as_mut_ptr().cast(),
                size_of_val(output),
                Some(&mut copied),
            )
        }
        .is_ok()
            && copied == size_of_val(output)
    }
    #[cfg(not(windows))]
    {
        let _ = address;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_ranges_never_form_references_and_empty_reads_cost_nothing() {
        assert!(words(0, &mut []));
        assert!(!words(0, &mut [0; 2]));
        assert!(!words(usize::MAX - 1, &mut [0; 2]));
        #[cfg(windows)]
        {
            let source = [16usize, 32];
            let mut destination = [0; 2];
            assert!(words(source.as_ptr() as usize, &mut destination));
            assert_eq!(source, destination);
        }
    }
}
