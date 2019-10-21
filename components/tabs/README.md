# Synced Tabs Component

![status-img](https://img.shields.io/static/v1?label=not%20implemented&message=Firefox%20Preview,%20Desktop,%20iOS&color=darkred)

## Implementation Overview
<TODO-ST>

## Directory structure
The relevant directories are as follows:

- `src`: The meat of the library. This contains cross-platform rust code that
  implements the synced of tabs.
- `ffi`: The Rust public FFI bindings. This is a (memory-unsafe, by necessity)
  API that is exposed to Kotlin and Swift. It leverages the `ffi_support` crate
  to avoid many issues and make it more safe than it otherwise would be.
  It uses protocol buffers for marshalling data over the FFI.
- `android`: This contains android bindings to synced tabs, written in Kotlin. These
  use JNA to call into to the code in `ffi`.
- `ios`: This contains the iOS binding to synced tabs, written in Swift. These use
  Swift's native support for calling code written in C to call into the code in
  `ffi`.

## Features
<TODO-ST>


## Business Logic

<TODO-ST>

## Getting started

**Prerequisites**: Firefox account authentication is necessary to obtain the keys to decrypt synced tabs data.  See the [android-components FxA Client readme](https://github.com/mozilla-mobile/android-components/blob/master/components/service/firefox-accounts/README.md) for details on how to implement on Android.  For iOS, Firefox for iOS still implement the legacy oauth.

**Platform-specific details**:
- <TODO-ST> Android
- iOS: start with the [guide to consuming rust components on iOS](https://github.com/mozilla/application-services/blob/master/docs/howtos/consuming-rust-components-on-ios.md)

## API Documentation
- TODO

## Testing

<TODO-ST>

## Telemetry
<TODO-ST>

## Examples
<TODO-ST>
