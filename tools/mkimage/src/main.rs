//! mkimage — build or refresh a FAT EFI System Partition image for LivingOS.
//!
//! QEMU's `vvfat` directory passthrough is convenient but unreliable for writes
//! (it can truncate files on commit), which is why Living Memory persistence
//! looked lossy. A real FAT disk image, mounted as a normal raw drive, persists
//! the kernel's writes faithfully across reboots.
//!
//!   mkimage <image.img> <bootx64.efi>
//!
//! If the image does not exist it is created (64 MiB) and formatted FAT. The
//! EFI loader is always (re)written to \EFI\BOOT\BOOTX64.EFI; any other files
//! the kernel created (e.g. livingos.mem) are preserved.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

const IMG_SIZE: u64 = 64 * 1024 * 1024;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: mkimage <image.img> <bootx64.efi>");
        std::process::exit(2);
    }
    let img_path = &args[1];
    let efi_path = &args[2];

    let fresh = !Path::new(img_path).exists();
    if fresh {
        let f = OpenOptions::new().read(true).write(true).create(true).truncate(true).open(img_path).expect("create image");
        f.set_len(IMG_SIZE).expect("size image");
        let mut f = f;
        fatfs::format_volume(&mut f, fatfs::FormatVolumeOptions::new()).expect("format FAT");
        println!("created + formatted {} ({} MiB)", img_path, IMG_SIZE / 1024 / 1024);
    }

    let f = OpenOptions::new().read(true).write(true).open(img_path).expect("open image");
    let fs = fatfs::FileSystem::new(f, fatfs::FsOptions::new()).expect("mount FAT");
    let root = fs.root_dir();
    let _ = root.create_dir("EFI");
    let efi = root.open_dir("EFI").expect("open EFI");
    let _ = efi.create_dir("BOOT");
    let boot = efi.open_dir("BOOT").expect("open EFI/BOOT");

    let data = std::fs::read(efi_path).expect("read efi");
    let mut file = boot.create_file("BOOTX64.EFI").expect("create BOOTX64.EFI");
    file.truncate().expect("truncate");
    file.write_all(&data).expect("write efi");
    file.flush().expect("flush");

    println!("wrote \\EFI\\BOOT\\BOOTX64.EFI ({} bytes){}", data.len(), if fresh { "" } else { "  (existing memory preserved)" });
}
