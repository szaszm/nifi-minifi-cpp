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

# minifi_pqdif

MiNiFi extension that reads **PQDIF** (Power Quality Data Interchange Format,
IEEE Std 1159.3) recordings and writes their observations as JSON.

Two crates, deliberately separated:

- **`pqdif/`** — the reader. Knows nothing about MiNiFi, depends on nothing from
  it, and is publishable on its own. See [`pqdif/README.md`](pqdif/README.md).
- **`.` (this crate)** — the adapter. Owns the JSON projection (`src/json.rs`)
  and the processor (`src/processors/parse_pqdif.rs`). Built as a `cdylib` and
  loaded by the agent like any other extension.

## `ParsePqdif`

| | |
| --- | --- |
| Input | One FlowFile whose content is a PQDIF file |
| Output | One FlowFile of JSON on `success`, or the original unchanged on `failure` |

### Properties

| Property | Default | Meaning |
| --- | --- | --- |
| Output Format | `ndjson` | `ndjson` — one JSON object per line. `array` — a single JSON array. |
| Checksum Verification | `whenPresent` | `always` / `whenPresent` / `never`. See below. |
| Include Sample Values | `true` | When false, omit each series' `values` but keep its `sample_count`. |

`Checksum Verification` defaults to `whenPresent` rather than `always` (which is
the `pqdif` library's own default) because a processor is pointed at whatever
files a site actually has. PQDIF has no "checksum absent" flag, so a writer that
never computes one simply leaves the field zero — common in the wild — and
`always` would reject those files outright. `whenPresent` decides per record, so
a file that checksums some records and not others is still fully checked wherever
a checksum exists. Set `always` for untrusted input and accept that some writers'
files will be rejected.

### Why one FlowFile out

A PQDIF file routinely holds hundreds of observations. Emitting one FlowFile per
observation would put that multiplier on every repository write, queue operation,
and provenance event downstream — whether or not the flow needs it. Instead both
output formats are splittable by a standard processor, so a flow that wants the
fan-out asks for it explicitly and pays for it at that point:

| Format | Split with |
| --- | --- |
| `ndjson` | `SplitText`, `Line Split Count` = 1 |
| `array` | `SplitJson`, `JsonPath Expression` = `$[*]` |

The objects are structurally identical between the two formats — only the
separators differ — so a consumer's field paths do not depend on which was
selected. A test asserts this.

The whole file is buffered in memory. That is forced rather than chosen: PQDIF is
a linked list of records addressed by absolute file offsets, so the parser needs
`Read + Seek`, while the API's `InputStream` is `BufRead` only.

### Output contract

One JSON object per observation. Keys come out in alphabetical order, which is
what `serde_json` gives without its `preserve_order` feature — stable and
diff-friendly, if not the order you would read them in. Below is a file that
declares everything; most files declare less.

```json
{
  "channels": [
    {
      "channel_definition_index": 0,
      "name": "Bus 1 Va",
      "phase": "AN",
      "phase_id": 1,
      "quantity_measured": "Voltage",
      "quantity_measured_id": 1,
      "series": [
        {
          "offset": 0.0,
          "quantity_characteristic": "Instantaneous",
          "quantity_characteristic_id": "a6b31add-b451-11d1-ae17-0060083a2628",
          "sample_count": 5,
          "scale": 1.0,
          "value_type": "Values",
          "value_type_id": "67f6af97-f753-11cf-9d89-0080c72e70a3",
          "values": [10.0, 20.0, 30.0, 40.0, 50.0]
        }
      ]
    }
  ],
  "data_source": "Recorder 4",
  "name": "OBS1",
  "observation_index": 0,
  "time_start": "2020-06-18T12:01:00"
}
```

Which keys appear depends on what the file declared. Omitted when the
corresponding tag is absent: `name` (on both the observation and the channel),
`data_source`, `time_create` / `time_start` / `time_triggered`, and each of the
four name/id pairs — `phase`, `quantity_measured`, `value_type`,
`quantity_characteristic`. Always present: `observation_index`, `channels`,
`channel_definition_index`, `series`, `scale`, `offset`, `sample_count`. And
`values`, unless `Include Sample Values` is false.

The rules behind that shape, each of which a test pins down:

- **Container (file-level) metadata is not in the content.** It goes to
  `pqdif.*` FlowFile attributes instead, because MiNiFi copies attributes onto
  every FlowFile derived from this one — so `pqdif.application` still identifies
  the writer after a downstream split, which is not true of anything embedded in
  the content. A wrapper object would also be dropped by `SplitJson`.
- **DataSource metadata is folded into the observations that reference it**,
  not emitted as a sibling record. The data source in force can change mid-file,
  and its per-channel phase and measured quantity are what make the samples
  interpretable — so those fields are merged onto the channel objects they
  describe. A sibling blob would be both harder to join and lost on split.
- **Enumerated ids carry both the resolved name and the raw id.** `"phase"` is
  what a person reads; `"phase_id"` is what survives a tag registry that has not
  been taught some vendor's extension.
- **Absent optional fields are omitted, not `null`.** PQDIF marks nearly
  everything optional and real files exercise that freely.
- **Timestamps are ISO 8601 and naive.** PQDIF records no zone, so appending `Z`
  would assert a UTC offset the file never claimed.
- **`scale` and `offset` are reported even though `values` are already scaled.**
  They document how the file encoded the data, which a consumer reconciling
  output against the original file needs.
- **Zero observations is a success, not an error.** A container-and-data-source
  file is legal PQDIF (a configured device that has not triggered). `ndjson`
  emits an empty body, `array` emits `[]` — each a valid document of its kind.

### Attributes written

`mime.type` (`application/x-ndjson` or `application/json`),
`pqdif.observation.count`, `pqdif.format`, plus whichever of the container
fields the file declared: `pqdif.writer.version`, `pqdif.application`,
`pqdif.author`, `pqdif.source.filename`, `pqdif.title`, `pqdif.notes`,
`pqdif.creation.time`, `pqdif.last.saved`, `pqdif.times.saved`.

Note `pqdif.source.filename` is the name the *writer* recorded inside the file,
which is not necessarily the `filename` MiNiFi picked it up under.

### Failure handling

Anything that makes the content unreadable as PQDIF — bad signature, truncation,
a corrupt record, a failed checksum, a cycle in the record chain — routes the
original FlowFile unchanged to `failure` and logs the cause. Partial output is
never emitted: a file that fails partway through its record chain would
otherwise hand downstream a silent subset of the recording.

A failure reading the session's own content stream is a different matter and is
fatal, since that is an engine-level problem rather than bad input.

## Building and testing

```bash
cargo test -p minifi_pqdif      # the extension: JSON contract + processor
cargo test -p pqdif             # the reader library
cargo clippy --all-targets
```

The processor tests drive the real parser over `pqdif`'s synthetic byte-array
fixtures (its `test-fixtures` feature), so they assert against bytes a PQDIF
reader genuinely accepts. No real `.pqd` files are vendored.

To build the extension into an agent, configure with `-DMINIFI_RUST=ON`.
