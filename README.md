# Slab

A desktop tool based on machine learning, developed out of personal interest.

## Development
1. install rust 
2. intall llvm for bindgen
2. install cargo-make: `cargo install cargo-make`
3. start dev : `cargo make dev`

## Disclaimer

### 1. Licensing and Commercial Use

The core creates of this project is licensed under the **GNU Affero General Public License v3.0 (AGPL-3.0)**.

* **Copyleft Requirement**: If you modify the software and provide its functionality as a service over a network, you must make your modified source code available to the users of that service.
* **Commercial Licensing**: Use of Slab-Core in closed-source commercial products without fulfilling AGPL obligations requires a separate **Commercial License**. Please contact the author for licensing inquiries.

### 2. AI-Generated Content (AICG) Responsibility

Slab is provided as a neutral inference tool and model orchestration engine.

* **Content Legality**: The user assumes all legal, moral, and ethical responsibilities for the output generated through the software (including but not limited to synthesized audio, transcribed text, and generated images).
* **Model Provenance**: The author does not verify the licensing or authorization of third-party models loaded by the user. Any copyright disputes arising from the use of unauthorized model weights are the sole responsibility of the user.

### 3. Hardware Safety and Performance

Slab interacts directly with system hardware (GPU/CPU) via frameworks such as CUDA, ROCm, and Metal.

* **Risk of Damage**: AI inference is a high-load task that may cause increased hardware temperature, power consumption, or driver instability. The author is not liable for any hardware failure, system crashes, or data loss resulting from the use of this software.
* **No Performance Guarantee**: Performance varies by hardware configuration. The author does not guarantee optimal performance or compatibility across all device models.

### 4. Data Privacy

* **Local-First Policy**: Slab is designed for local inference. Unless explicitly configured by the user to use external APIs, Slab does not upload raw audio, text, or model data to any third-party servers.
* **Telemetry**: Any future telemetry features for performance optimization will be strictly opt-in and disclosed to the user.

### 5. No Warranty

> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE, AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES, OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT, OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE.

### 6. Compliance with Local Regulations
Any country, organization, or individual using this software must comply with the laws and regulations of their respective jurisdiction. Users are responsible for ensuring that the AI models they load and utilize are duly recognized by their local government or comply with relevant regulatory requirements. The use of this software for any activity that violates laws, administrative regulations, or infringes upon the legal rights of others is strictly prohibited.

## License

Copyright (c) Cyberhan123.

This repository is multi-licensed by component.

Apache-2.0:
- Repository root files (unless otherwise stated)
- `slab-app`
- `slab-proto`
- `slab-diffusion-sys`
- `slab-llama-sys`
- `slab-whisper-sys`
- `slab-ggml-sys`
- `slab-types`

See [LICENSE](./LICENSE).

AGPL-3.0-only:
- `slab-core`
- `slab-core-macros`
- `slab-diffusion`
- `slab-libfetch`
- `slab-llama`
- `slab-runtime`
- `slab-server`
- `slab-whisper`
- `slab-agent`
- `slab-ggml`
- `slab-build-utils`

Each AGPL component contains its own `LICENSE` file in that directory.

# AI Usage Restriction:
The author reserves all rights to the software. No individual or third party may use any artificial intelligence technology to imitate or tamper with the software or use it for commercial purposes without the author's express written consent.