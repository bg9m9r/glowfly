//! The packaging checks, one test binary: the app icon and installer art against their drawing
//! (`app_icon`; `UPDATE_ICONS=1` redraws them), the Linux packages' file lists (`linux_packages`),
//! the Flatpak manifest (`flatpak_package`), the package manager manifests (`package_manifests`)
//! and the Release workflow (`release_workflow`).

mod app_icon;
mod flatpak_package;
mod linux_packages;
mod package_manifests;
mod release_workflow;
