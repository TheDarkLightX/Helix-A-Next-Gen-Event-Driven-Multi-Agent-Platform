# Third-Party Licenses

This document contains the licenses for third-party software used in the Helix Platform.

## RISC Zero zkVM

**Repository**: https://github.com/risc0/risc0  
**License**: Apache-2.0  
**Usage**: Zero-knowledge virtual machine capabilities (when enabled)

```
Copyright 2024 RISC Zero, Inc.

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
```

**Note**: RISC Zero zkVM integration is currently implemented as mock/placeholder code. When activated, the following crates will be used:
- `risc0-zkvm` - Core zkVM functionality
- `risc0-build` - Build system integration

## License Compatibility

All third-party dependencies are compatible with the Apache-2.0 license used by the Helix Platform.

## Attribution Requirements

When distributing software that includes RISC Zero components, ensure:
1. Include this license notice
2. Preserve copyright notices
3. Include the Apache-2.0 license text
4. Document any modifications made to RISC Zero code

## Contact

For license questions or concerns, please contact the Helix Platform maintainers.
