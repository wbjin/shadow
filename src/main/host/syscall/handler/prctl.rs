use linux_api::errno::Errno;
use linux_api::prctl::{ArchPrctlOp, PrctlOp};
use linux_api::sched::SuidDump;
use shadow_shim_helper_rs::syscall_types::ForeignPtr;

use crate::host::syscall::handler::{SyscallContext, SyscallHandler};
use crate::host::syscall::types::SyscallError;

impl SyscallHandler {
    log_syscall!(
        prctl,
        /* rv */ std::ffi::c_int,
        /* option */ PrctlOp,
        /* arg2 */ std::ffi::c_ulong,
        /* arg3 */ std::ffi::c_ulong,
        /* arg4 */ std::ffi::c_ulong,
        /* arg5 */ std::ffi::c_ulong,
    );
    pub fn prctl(
        ctx: &mut SyscallContext,
        option: PrctlOp,
        arg2: std::ffi::c_ulong,
        _arg3: std::ffi::c_ulong,
        _arg4: std::ffi::c_ulong,
        _arg5: std::ffi::c_ulong,
    ) -> Result<std::ffi::c_int, SyscallError> {
        match option {
            PrctlOp::PR_CAP_AMBIENT
            | PrctlOp::PR_CAPBSET_READ
            | PrctlOp::PR_CAPBSET_DROP
            | PrctlOp::PR_SET_CHILD_SUBREAPER
            | PrctlOp::PR_GET_CHILD_SUBREAPER
            | PrctlOp::PR_SET_ENDIAN
            | PrctlOp::PR_GET_ENDIAN
            | PrctlOp::PR_SET_FP_MODE
            | PrctlOp::PR_GET_FP_MODE
            | PrctlOp::PR_SET_FPEMU
            | PrctlOp::PR_GET_FPEMU
            | PrctlOp::PR_SET_FPEXC
            | PrctlOp::PR_GET_FPEXC
            | PrctlOp::PR_SET_KEEPCAPS
            | PrctlOp::PR_GET_KEEPCAPS
            | PrctlOp::PR_MCE_KILL
            | PrctlOp::PR_MCE_KILL_GET
            | PrctlOp::PR_MPX_ENABLE_MANAGEMENT
            | PrctlOp::PR_MPX_DISABLE_MANAGEMENT
            | PrctlOp::PR_SET_NAME
            | PrctlOp::PR_GET_NAME
            | PrctlOp::PR_SET_NO_NEW_PRIVS
            | PrctlOp::PR_GET_NO_NEW_PRIVS
            | PrctlOp::PR_SET_MM
            | PrctlOp::PR_SET_PTRACER
            | PrctlOp::PR_SET_SECUREBITS
            | PrctlOp::PR_GET_SECUREBITS
            | PrctlOp::PR_GET_SPECULATION_CTRL
            | PrctlOp::PR_SET_THP_DISABLE
            | PrctlOp::PR_TASK_PERF_EVENTS_DISABLE
            | PrctlOp::PR_TASK_PERF_EVENTS_ENABLE
            | PrctlOp::PR_GET_THP_DISABLE
            | PrctlOp::PR_GET_TIMERSLACK
            | PrctlOp::PR_SET_TIMING
            | PrctlOp::PR_GET_TIMING
            | PrctlOp::PR_GET_TSC
            | PrctlOp::PR_GET_UNALIGN => {
                log::trace!("prctl {option} executing natively");
                Err(SyscallError::Native)
            }
            PrctlOp::PR_SET_SECCOMP | PrctlOp::PR_GET_SECCOMP => {
                log::warn!("Not allowing seccomp prctl {option}");
                Err(Errno::EINVAL.into())
            }
            // Needs emulation to have the desired effect, but also N/A on x86_64.
            PrctlOp::PR_SET_UNALIGN
            // Executing natively could interfere with shadow's interception of rdtsc. Needs
            // emulation.
            | PrctlOp::PR_SET_TSC
            // Executing natively wouldn't directly hurt anything, but wouldn't have the desired
            // effect.
            | PrctlOp::PR_SET_TIMERSLACK
            // Wouldn't actually hurt correctness, but could significantly hurt performance.
            | PrctlOp::PR_SET_SPECULATION_CTRL
            // We use this signal to ensure managed processes die when Shadow does. Allowing the
            // process to override it could end up allowing orphaned managed processes to live on
            // after shadow exits.
            | PrctlOp::PR_SET_PDEATHSIG => {
                log::warn!("Not allowing unimplemented prctl {option}");
                Err(Errno::EINVAL.into())
            }
            PrctlOp::PR_GET_TID_ADDRESS => {
                let out_ptr = ForeignPtr::from(arg2)
                    .cast::<ForeignPtr<linux_api::posix_types::kernel_pid_t>>();
                let tid_addr = ctx.objs.thread.get_tid_address();
                ctx.objs.process.memory_borrow_mut().write(out_ptr, &tid_addr)?;
                Ok(0)
            }
            PrctlOp::PR_SET_DUMPABLE => {
                let dumpable = SuidDump::new(arg2.try_into().or(Err(Errno::EINVAL))?);
                if [SuidDump::SUID_DUMP_DISABLE, SuidDump::SUID_DUMP_USER].contains(&dumpable) {
                    ctx.objs.process.set_dumpable(dumpable);
                    Ok(0)
                } else {
                    Err(Errno::EINVAL.into())
                }
            }
            PrctlOp::PR_GET_DUMPABLE => {
                Ok(ctx.objs.process.dumpable().val())
            }
            _ => {
                log::warn!("Unknown prctl operation {option}");
                Err(Errno::EINVAL.into())
            }
        }
    }

    log_syscall!(
        arch_prctl,
        /* rv */ std::ffi::c_ulong,
        /* option */ ArchPrctlOp,
        // Sometimes a pointer; sometimes a small integer, depending on the
        // operation. Probably most conveniently formatted as a pointer.
        /* value */
        *const std::ffi::c_void,
    );
    pub fn arch_prctl(
        _ctx: &mut SyscallContext,
        option: ArchPrctlOp,
        _arg: std::ffi::c_ulong,
    ) -> Result<std::ffi::c_ulong, SyscallError> {
        match option {
            ArchPrctlOp::ARCH_GET_CPUID => {
                // Always *say* that cpuid is allowed. The shim actually
                // arranges to trap and emulate it, but the managed program
                // doesn't need to know that.
                Ok(1u64)
            }
            ArchPrctlOp::ARCH_SET_CPUID => {
                // Don't allow the managed program to try trapping cpuid itself.
                // arch_prctl(2): "ENODEV: ARCH_SET_CPUID was requested, but the underlying hardware does not support CPUID faulting."
                //
                // It could be reasonable to return Ok(0) here when the argument
                // is 1 (i.e. to allow cpuid, which would be a no-op), but
                // experimentally linux returns ENODEV here in that case too.
                // (At least in shadow's github CI, though it's possible that
                // docker or something is intercepting the arch_prctl and we're
                // inadvertently modeling *that* behavior.)
                Err(Errno::ENODEV.into())
            }
            ArchPrctlOp::ARCH_SET_FS
            | ArchPrctlOp::ARCH_GET_FS
            | ArchPrctlOp::ARCH_SET_GS
            | ArchPrctlOp::ARCH_GET_GS => {
                // Potentially used by threading libraries. Allow natively.
                Err(SyscallError::Native)
            }
            x => {
                // The kernel headers (and ArchPrctlOp) have more values defined
                // that aren't in the libc man pages.  It'd take some work to
                // hunt down corresponding documentation and figure out how
                // they'd interact with shadow. In the meantime conservatively
                // block everything else.
                log::warn!("Unknown or unsupported arch_prctl operation {x:?}");
                Err(Errno::EINVAL.into())
            }
        }
    }
}
