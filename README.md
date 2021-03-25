# Borealis

**NOTE**: This is not being actively developed. If you are looking for a RGB solution, I suggest [OpenRGB](https://gitlab.com/CalcProgrammer1/OpenRGB).

Borealis is an Asus Aura Sync driver application for Linux. It can set
your peripherals' RGB lighting without the use of the Windows Aura application.

The _Aura_ branding covers multiple protocols. At present, Borealis only 
supports the motherboard-based SMBus/I2C protocol, which provides control
over LED lighting on the motherboard and RAM (e.g. G.Skill Trident Z RGB).
It may support additional forms of lighting, including Aura-enabled GPUs
and input peripherals, in the future.

Borealis has only been tested on my personal workstation (Arch Linux,
X399 Strix-E, 4x G.Skill Trident Z RGB, Lian Li Bora Lite fans), and makes 
certain assumptions about where to locate resources. While these should be 
valid across similar systems to mine, I have not tested them, and I make _no
guarantees_. I'm not responsible for your computer blowing up, but I'm
happy to help get it working if it's within my purview.

## Building

Install the latest version of [Rust](https://www.rust-lang.org/), clone this
repository, and run `cargo build` within the directory to produce binaries.

## Running

Currently, Borealis only supports setting all LEDs to a given colour. This
interface will be extended in future to provide for additional control.

As Borealis uses Linux's I2C interface, you will need to ensure that this
has been loaded. To do so temporarily, you can use `modprobe i2c-dev`;
for extended use, consider having the module 
[automatically loaded](https://wiki.archlinux.org/index.php/Kernel_module).

To run Borealis, use `cargo run` or run the binary built by `cargo build`.
Arguments are a RGB triplet - that is,
    
    cargo run 127 0 127

to set all lighting on your motherboard to purple.

### Caveats
* As previously mentioned, this does not support all Aura products. Other
  products include STRIX GPUs, non-addressable RGB LED strips, keyboards,
  mice, and potentially more. 

  Supporting these will take additional reverse engineering, which may or
  may not happen in the future.

* You may not have the same memory layout as I do, so Borealis may fail
  to connect to some of the RAM sticks. Adjust the `controllers` array
  in `src/main.rs` to suit; there will be better detection of installed
  devices in the future.

* Annoyingly, the I2C addresses for the individual RAM sticks may not be
  available from a cold boot. Using Aura Sync in Windows _may_ make them
  visible, but it's not guaranteed. I hope to make this more reliable in
  future.

* The Aura Sync controller for the motherboard resides on an auxiliary SMBus.
  For AMD systems, this SMBus is not initialised with the stock Linux kernel;
  to get it to work, you will need to patch your kernel. The patch follows for
  Linux kernel 4.20
  (derived from 
  [here](https://gitlab.com/CalcProgrammer1/KeyboardVisualizer/issues/85#note_121577579)):
```diff
--- a/drivers/i2c/busses/i2c-piix4.c	2019-01-09 01:23:06.197945763 +1100
+++ b/drivers/i2c/busses/i2c-piix4.c	2019-01-09 01:24:58.007942622 +1100
@@ -964,6 +964,11 @@
 		retval = piix4_setup_sb800(dev, id, 1);
 	}
 
+	if (dev->vendor == PCI_VENDOR_ID_AMD &&
+	    dev->device == PCI_DEVICE_ID_AMD_KERNCZ_SMBUS) {
+		retval = piix4_setup_sb800(dev, id, 1);
+	}
+
 	if (retval > 0) {
 		/* Try to add the aux adapter if it exists,
 		 * piix4_add_adapter will clean up if this fails */
```

## Acknowledgements
Many, many thanks to the great work of those at
https://gitlab.com/CalcProgrammer1/KeyboardVisualizer/issues/85
, whose extensive reverse-engineering work paved the way for me to investigate
further and determine reliable methods of communicating with Aura devices.
I wouldn't have been able to get started without their efforts demonstrating
the viability of the approach.

Additionally, much amusement was derived from this 
[Aura Sync CVE](https://seclists.org/fulldisclosure/2018/Dec/34). Folks, the
way Aura Sync is implemented on Windows is really, really bad. I hope they fix
it soon.
