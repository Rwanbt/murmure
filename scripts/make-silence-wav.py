#!/usr/bin/env python3
"""Write a deterministic recording that holds no speech.

The counter-test of the transcription needs a file that carries room noise
and nothing else. A file of pure zeros is not representative: a real
microphone always returns a little hiss, and that hiss is exactly what makes
a model hallucinate words. This script therefore writes low-amplitude noise
with a fixed seed, so the file is identical on every run.

Usage: make-silence-wav.py <output.wav>
"""

import random
import struct
import sys
import wave

SAMPLE_RATE = 16_000
SECONDS = 5
# Peak amplitude of the noise, as a fraction of full scale. The round-trip
# test treats anything below 0.05 as silence, and a real microphone in a
# quiet room sits far below that.
PEAK = 0.02
SEED = 20260821


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__, file=sys.stderr)
        return 2

    generator = random.Random(SEED)
    limit = int(PEAK * 32_767)
    frames = bytearray()
    for _ in range(SAMPLE_RATE * SECONDS):
        frames += struct.pack("<h", generator.randint(-limit, limit))

    with wave.open(sys.argv[1], "wb") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(SAMPLE_RATE)
        handle.writeframes(bytes(frames))

    print(f"wrote {SECONDS}s of silence at {SAMPLE_RATE} Hz, peak {PEAK:.0%} of full scale")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
