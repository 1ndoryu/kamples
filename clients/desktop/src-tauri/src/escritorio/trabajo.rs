/* Job Object de Windows: los hijos se mueren con el lanzador, pase lo que pase.
 *
 * [por que] El cierre ordenado (`RunEvent::ExitRequested`) solo cubre la salida
 * normal. Si el proceso se cae, se cuelga o lo matan desde el administrador de
 * tareas, el `Drop` de Rust NO corre y quedaban vivos `glory-backend.exe` (que
 * seguía sirviendo en 4317 impidiendo el siguiente arranque) y `postgres.exe`
 * (escribiendo en el cluster). El Job Object lo resuelve en el kernel: Windows
 * mata todo el grupo cuando se cierra el último handle del job, y ese handle es
 * este proceso.
 *
 * [por que no basta el «hijo directo»] `postgres.exe` lanza sus propios
 * procesos (`--forkchild=checkpointer`, `bgworker`…). Al asignar el padre al
 * job, todos sus descendientes heredan la pertenencia.
 */

#[cfg(windows)]
pub struct Trabajo(isize);

/* [por que Send a mano] `HANDLE` es un puntero opaco que windows-sys declara
 * sin `Send`, pero un handle del kernel se puede usar desde cualquier hilo: es
 * justo lo que hacen las APIs. Tauri exige `Send` para guardar el servicio en
 * su estado, y lo que viaja entre hilos es el identificador, no memoria. */
unsafe impl Send for Trabajo {}
unsafe impl Sync for Trabajo {}

#[cfg(windows)]
impl Trabajo {
    /// Crea el job con la regla de matar todo al cerrarse.
    pub fn nuevo() -> Option<Self> {
        use windows_sys::Win32::System::JobObjects::{
            CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };

        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return None;
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let bien = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const std::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            if bien == 0 {
                let _ = windows_sys::Win32::Foundation::CloseHandle(job);
                return None;
            }
            Some(Self(job as isize))
        }
    }

    /// Mete un proceso ya lanzado dentro del job.
    ///
    /// Si falla no es fatal: se queda el cierre ordenado, que cubre la salida
    /// normal. No se propaga el error para no impedir arrancar la app por esto.
    pub fn asignar(&self, hijo: &std::process::Child) {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;

        let job = self.0 as windows_sys::Win32::Foundation::HANDLE;
        let proceso = hijo.as_raw_handle() as windows_sys::Win32::Foundation::HANDLE;
        unsafe {
            AssignProcessToJobObject(job, proceso);
        }
    }
}

#[cfg(windows)]
impl Drop for Trabajo {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0 as windows_sys::Win32::Foundation::HANDLE);
        }
    }
}

/* Fuera de Windows no hay job objects: el cierre ordenado basta. */
#[cfg(not(windows))]
pub struct Trabajo;

#[cfg(not(windows))]
impl Trabajo {
    pub fn nuevo() -> Option<Self> {
        Some(Self)
    }

    pub fn asignar(&self, _hijo: &std::process::Child) {}
}
