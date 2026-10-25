# 0013: Devices (UDI) to Device

Status: accepted. No IG page covers devices; this follows C-CDA Product Instance (4.37) and the US Core Implantable Device
profile.

- A Product Instance participant (found anywhere in the statement tree) becomes a Device with `udiCarrier.carrierHRF` taken
  from the id under the FDA UDI root, and `type` from the playing device code. A GS1 UDI starting `(01)` also gives
  `deviceIdentifier`. The same UDI seen twice is one Device.
- `status` is left out: nothing in the document says whether the device is still in use.
- `us-core-implantable-device` is claimed when carrierHRF and type are present.
