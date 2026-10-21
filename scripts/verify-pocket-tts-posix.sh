#!/usr/bin/env bash
# Verifies the pinned Pocket TTS runtime on Linux and macOS.
#
# The official Kyutai release publishes a build for five targets. This script
# installs the one that matches the current machine, verifies its published
# digest before anything is executed, checks that the generation interface is
# the one this project relies on, and produces a French WAV that the shared
# structural validator accepts.
#
# The archive digest is the value published in the SHA256SUMS of the release,
# never a value recomputed locally: a script that trusts whatever it just
# downloaded proves nothing.
#
# Usage:
#   ./scripts/verify-pocket-tts-posix.sh [-d DESTINATION] [-k]
#
#   -d  Directory the runtime is installed into (default: a temporary one)
#   -k  Keep the generated WAV, so another test can consume it

set -euo pipefail

readonly RELEASE='v4.0.0'
readonly BASE_URI="https://github.com/kyutai-labs/pocket-tts/releases/download/${RELEASE}"
readonly WAVE_VALIDATOR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/verify-pocket-tts-wave.py"

destination=''
keep_wav=0

while getopts ':d:kh' option; do
    case "${option}" in
        d) destination="${OPTARG}" ;;
        k) keep_wav=1 ;;
        h)
            sed -n '2,20p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        :)
            echo "Option -${OPTARG} requires a value." >&2
            exit 2
            ;;
        \?)
            echo "Unknown option -${OPTARG}." >&2
            exit 2
            ;;
    esac
done

if [[ -z "${destination}" ]]; then
    destination="$(mktemp -d "${TMPDIR:-/tmp}/murmure-pocket-tts.XXXXXX")"
fi
mkdir -p "${destination}"
destination="$(cd "${destination}" && pwd)"

# The target this machine needs, and the digest that the release publishes for
# it. A digest that is not known here means the target was never qualified, so
# the script refuses to run rather than trusting an unknown binary.
target_of() {
    local system machine
    system="$(uname -s)"
    machine="$(uname -m)"
    case "${system}:${machine}" in
        Linux:x86_64) echo 'x86_64-unknown-linux-gnu' ;;
        Linux:aarch64 | Linux:arm64) echo 'aarch64-unknown-linux-gnu' ;;
        Darwin:x86_64) echo 'x86_64-apple-darwin' ;;
        Darwin:arm64) echo 'aarch64-apple-darwin' ;;
        *) echo "unsupported:${system}:${machine}" ;;
    esac
}

# A portable digest, because Linux and macOS ship different tools for it.
sha256_of() {
    if command -v sha256sum > /dev/null 2>&1; then
        sha256sum -- "$1" | cut -d' ' -f1
    elif command -v shasum > /dev/null 2>&1; then
        shasum -a 256 -- "$1" | cut -d' ' -f1
    else
        echo 'FAIL: neither sha256sum nor shasum is available to verify the archive.' >&2
        return 1
    fi
}

digest_of() {
    case "$1" in
        x86_64-unknown-linux-gnu) echo '1336a77e60ce11650a5a3dc354770155a345bfa797f7a05d10bc5b1bcfe0d7e3' ;;
        aarch64-unknown-linux-gnu) echo '644a9478d7cbd7d432f5a4898007aa7d178a897a65aef7ebc64557e0b1a1739a' ;;
        x86_64-apple-darwin) echo '3a908c524d1ca976eb5f699c523fbdd47a5fa029ccc3bd9f6197b8f8d91f01c1' ;;
        aarch64-apple-darwin) echo 'e209feca849d53df18eba89f5bd4c253c1be62a6c54794db5654635c95e02990' ;;
        *) echo '' ;;
    esac
}

target="$(target_of)"
case "${target}" in
    unsupported:*)
        echo "FAIL: Pocket TTS has no pinned build for ${target#unsupported:}." >&2
        exit 1
        ;;
esac

expected_digest="$(digest_of "${target}")"
if [[ -z "${expected_digest}" ]]; then
    echo "FAIL: the digest of ${target} is not pinned, so it was never qualified." >&2
    exit 1
fi

archive="pocket-tts-3.3.0-${target}.tar.gz"
uri="${BASE_URI}/${archive}"
archive_path="${destination}/${archive}"

echo "Retrieving the pinned Pocket TTS release ${RELEASE} for ${target}."
curl --fail --silent --show-error --location --retry 3 --output "${archive_path}" "${uri}"

# The digest is verified before anything is extracted or executed.
actual_digest="$(sha256_of "${archive_path}")"
if [[ "${actual_digest}" != "${expected_digest}" ]]; then
    rm -f "${archive_path}"
    echo "FAIL: the archive does not match the published digest. Expected '${expected_digest}' but got '${actual_digest}'. Nothing was executed." >&2
    exit 1
fi
echo "PASS: archive digest matches the value published in SHA256SUMS."

expanded="${destination}/expanded"
rm -rf "${expanded}"
mkdir -p "${expanded}"
tar --extract --gzip --file "${archive_path}" --directory "${expanded}"

# The name of the executable is not assumed: the release decides it, so the
# script looks for the single executable that was extracted.
binary=''
while IFS= read -r candidate; do
    if [[ -z "${binary}" ]]; then
        binary="${candidate}"
    else
        echo "FAIL: the archive contains more than one candidate executable." >&2
        exit 1
    fi
done < <(find "${expanded}" -type f -perm -u+x -name 'pocket-tts*' ! -name '*.tar.gz')

if [[ -z "${binary}" ]]; then
    echo "FAIL: the archive does not contain the Pocket TTS executable." >&2
    exit 1
fi
echo "Extracted executable: ${binary}"
echo "PASS: executable SHA256 $(sha256_of "${binary}")"

# The interface this project depends on is verified, not assumed.
"${binary}" --help > /dev/null
"${binary}" generate --help > /dev/null
generate_help="$("${binary}" generate --help)"
for flag in --language --voice --text --device --output-path; do
    if ! grep -q -- "${flag}" <<< "${generate_help}"; then
        echo "FAIL: the pinned runtime does not support ${flag}." >&2
        exit 1
    fi
done
echo "PASS: the pinned runtime exposes the generation interface."

# Fixed test text. Personal content never goes through continuous integration.
readonly TEST_TEXT='Bonjour, ceci est une verification du moteur vocal francais.'
wav="${destination}/french-smoke.wav"
rm -f "${wav}"

echo "Running a fixed French synthesis on the CPU, without voice cloning."
printf '%s' "${TEST_TEXT}" | "${binary}" generate \
    --language french \
    --voice estelle \
    --device cpu \
    --text - \
    --output-path "${wav}" \
    --quiet

if [[ ! -f "${wav}" ]]; then
    echo "FAIL: the runtime did not produce the expected WAV file." >&2
    exit 1
fi

# The structure and the samples are checked by the shared validator, so the
# same contract applies to every platform.
python3 "${WAVE_VALIDATOR}" --path "${wav}" --minimum-seconds 0.5 --maximum-seconds 30
echo "PASS: the generated French WAV is structurally valid and carries audio."

if [[ "${keep_wav}" -eq 1 ]]; then
    echo "Generated WAV kept at: ${wav}"
    echo "Executable: ${binary}"
else
    rm -f "${wav}"
fi

echo "PASS: Pocket TTS ${RELEASE} is verified on ${target}."
