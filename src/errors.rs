use crate::layers::pip::PipLayerError;
use crate::layers::python::PythonLayerError;
use crate::python_version::PythonVersionError;
// use indoc::formatdoc;
// use libherokubuildpack::log_error;

#[derive(Debug)]
pub(crate) enum PythonBuildpackError {
    PipLayer(PipLayerError),
    PythonLayer(PythonLayerError),
    PythonVersion(PythonVersionError),
}

// pub(crate) fn on_python_buildpack_error(buildpack_error: PythonBuildpackError) -> i32 {
//     match buildpack_error {
//         PythonBuildpackError::RuntimeLayerError(inner) => match inner {
//             RuntimeLayerError::DownloadUnpackFailed(download_error) => log_error(
//                 "Runtime installation failed",
//                 formatdoc! {"
//                         An error occurred while installing the Python runtime. In some cases,
//                         this happens due to an unstable network connection. Please try again and see
//                         if the error resolves itself.

//                         {download_error}
//                     ", download_error = download_error},
//             ),
//         },
//     }

//     1
// }

impl From<PythonBuildpackError> for libcnb::Error<PythonBuildpackError> {
    fn from(error: PythonBuildpackError) -> Self {
        Self::BuildpackError(error)
    }
}
