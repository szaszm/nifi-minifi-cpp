<!--
  Licensed to the Apache Software Foundation (ASF) under one or more
  contributor license agreements.  See the NOTICE file distributed with
  this work for additional information regarding copyright ownership.
  The ASF licenses this file to You under the Apache License, Version 2.0
  (the "License"); you may not use this file except in compliance with
  the License.  You may obtain a copy of the License at
      http://www.apache.org/licenses/LICENSE-2.0
  Unless required by applicable law or agreed to in writing, software
  distributed under the License is distributed on an "AS IS" BASIS,
  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
  See the License for the specific language governing permissions and
  limitations under the License.
-->

# Attribution — `tag_definitions.xml`

`tag_definitions.xml` in this directory is a verbatim copy of `TagDefinitions.xml`
from Grid Protection Alliance's GSF (Grid Solutions Framework) project. It is
the authoritative registry of tag GUIDs, tag hierarchy, and enumerated tag values
that make up the PQDIF (Power Quality Data Interchange Format, IEEE Std 1159.3)
data model.

- **Upstream repository:** <https://github.com/GridProtectionAlliance/gsf>
- **Upstream path:** `Source/Libraries/GSF.PQDIF/TagDefinitions.xml`
- **Pinned commit:** `4fd876c39d838089ab36e6c98efc946650c23770` (2016-04-06)
- **SHA-256:** `36fde38aae119e63284f3fc30aac25198b791e5d2a2283bf3200e4b1548a7493`
- **Size:** 201,037 bytes, 4,367 lines

The file has not been functionally modified in the upstream repository since
2016-04-06 — the pinned commit is effectively the latest revision. The XML
itself carries no in-file license header; its license is governed by the
repository-level MIT license, whose text is preserved verbatim alongside this
file as `LICENSE-GSF-PQDIF`.

## License

GSF is distributed under the MIT License (Copyright (c) 2015 Grid Protection
Alliance). MIT is a Category A license under the Apache Software Foundation
3rd-party license policy and is compatible with Apache-2.0 redistribution.
See `LICENSE-GSF-PQDIF` in this directory for the full license text.

## Modernized successor

Grid Protection Alliance is migrating GSF's subprojects into individually
maintained packages under the `gemstone/` organization. The successor to
`GSF.PQDIF` is <https://github.com/gemstone/pqdif> (`Gemstone.PQDIF`,
distributed on NuGet). Both projects contain byte-identical copies of
`TagDefinitions.xml` (`gemstone/pqdif` at `src/Gemstone.PQDIF/TagDefinitions.xml`),
so pinning against either is equivalent. GSF was chosen here as the stabler
upstream given that gemstone/pqdif is more recently reorganized.
