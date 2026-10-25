# 0042. UDI fields and Procedure.focalDevice

Status: accepted

## Decision
- A GS1 UDI in human readable form is read for the manufacture date (11), expiration date (17),
  lot (10) and serial (21). They go to `Device.manufactureDate`, `expirationDate`, `lotNumber`
  and `serialNumber`. A day of `00` gives a partial date (`yyyy-mm`); an invalid date is left out.
  Non-GS1 UDIs keep the carrier only.
- A procedure with a Product Instance participant gets `focalDevice.manipulated` pointing at the
  Device. No `action` is set, because the document does not say what was done to the device.
- Device ids are allocated per UDI the first time anything asks for them, so a procedure can
  reference a device that is mapped later without changing the mapper order.
