#!/bin/sh -eu

################################################################################
## This script shall conform to the POSIX.1 standard, a.k.a. IEEE Std 1003.1. ##
## When utilities defined in the standard are to be invoked, they shall only  ##
## be invoked utilizing functions defined by the standard, excluding any and  ##
## all extensions to the standard functions, e.g. GNU extensions.             ##
##                                                                            ##
## Version of the POSIX.1 standard used: POSIX.1-2008                         ##
## https://pubs.opengroup.org/onlinepubs/9699919799.2008edition/              ##
##                                                                            ##
## Used version of the standard should not be moved forward unless necessary  ##
## in order to keep the script as portable as possible between different      ##
## environments.                                                              ##
##                                                                            ##
## Used version of the standard should be moved backwards if possible in      ##
## order to keep the script as portable as possible between different         ##
## environments.                                                              ##
################################################################################

set -eu

error() {
  set -eu

  case "${#:?}" in
    ("1")
      "tee" \
        "/artifacts/error-report.txt" \
        >&2 \
        <<EOF
${1:?}
EOF
      ;;
    (*) "error" "\"error\" function requires exactly one argument!"
  esac

  exit 1
}

readonly CHECK_DEPENDENCIES_UPDATED

case "${CHECK_DEPENDENCIES_UPDATED:?}" in
  ("false") ;;
  (*)
    if ! "cargo" \
      "update" \
      --locked
    then
      "error" "Updating dependencies failed!"
    fi
esac

RUSTFLAGS="${RUSTFLAGS:+"${RUSTFLAGS:?} "}-C link-arg=-s"
readonly RUSTFLAGS
export RUSTFLAGS

if SOFTWARE_RELEASE_ID="$("cat" "/release-version.txt")"
then
  readonly SOFTWARE_RELEASE_ID

  : "${SOFTWARE_RELEASE_ID:?"Release version cannot be null!"}"

  export SOFTWARE_RELEASE_ID
else
  "error" "Failed to read release version!"
fi

if cosmwasm_capabilities="$("cat" "/configuration/cosmwasm_capabilities")"
then
  readonly cosmwasm_capabilities
else
  "error" "Failed to read available CosmWasm capabilities!"
fi

build_profile="${1:?"Passing build profile as first parameter is required!"}"
readonly build_profile

shift

case "${#:?}" in
  ("0") ;;
  (*) "error" "Expected only one argument denominating build profile!"
esac

if mapped_build_profile="$("cat" "/build-profiles/${build_profile:?}")"
then
  readonly mapped_build_profile

  : "${mapped_build_profile:?"Mapped build profile cannot be null!"}"
else
  if build_profiles="$(
    "ls" \
      -1 \
      "/build-profiles/"
  )"
  then
    if build_profiles="$(
      "sed" \
        -e "s/^/* /"\
        <<EOF
${build_profiles:?}
EOF
    )"
    then
      readonly build_profiles

      "error" "Failed to read build profile mapping!

Existing profiles:
${build_profiles:?}"
    else
      "error" "Failed to invoke \"sed\"!"
    fi
  else
    "error" "Failed to read available build profiles!"
  fi
fi

if max_binary_size="$(
  "cat" "/configuration/${build_profile:?}-max-binary-size"
)"
then
  readonly max_binary_size

  : "${max_binary_size:?"Maximum binary size cannot be null!"}"
else
  "error" "Failed to read max binary size for build profile!"
fi

if ! working_directory="$("pwd")"
then
  "error" "Failed to retrieve current directory via \"pwd\"!"
fi
if ! working_directory="${working_directory%%"/"}"
then
  "error" "Failed to strip prefix from current directory!"
fi
if ! working_directory="${working_directory##"/"}"
then
  "error" "Failed to strip prefix from current directory!"
fi
readonly working_directory

case "${working_directory:?}" in
  ("platform")
    dex_type=""
    ;;
  ("protocol")
    protocol_json="/build-configuration/protocol.json"
    readonly protocol_json

    topology_json="/build-configuration/topology.json"
    readonly topology_json

    for file in \
      "${protocol_json:?}" \
      "${topology_json:?}"
    do
      if ! test \
        "(" \
        -f "${file:?}" \
        ")" \
        -a \
        "(" \
        -r "${file:?}" \
        ")"
      then
        "error" "\"${file:?}\" doesn't exist or is not readable!"
      fi
    done

    if protocol="$(
      "jq" \
        -c \
        "." \
        <"${protocol_json:?}"
    )"
    then
      readonly protocol
    else
      "error" "Failed to read protocol describing JSON file!"
    fi

    if dex_type="$(
      "jq" \
        --exit-status \
        --raw-output \
        --argjson "protocol" "${protocol:?}" \
        ".networks[\$protocol.dex_network].dexes[\$protocol.dex].type | \
select(. != null)" \
        <"${topology_json:?}"
    )"
    then
      readonly dex_type

      : "${dex_type:?"DEX type cannot be null!"}"
    else
      "error" "Failed to get DEX type from topology describing JSON file!"
    fi
    ;;
  (*) "error" "Current directory corresponds to an unknown workspace!"
esac
readonly dex_type
: "${dex_type?}"

if ! contracts_count="$(
  "cat" "/configuration/${working_directory:?}-contracts-count"
)"
then
  "error" "Failed to read expected contracts count configuration for workspace!"
fi
readonly contracts_count
: "${contracts_count:?"Contracts count cannot be null!"}"

for directory in \
  "target" \
  "artifacts" \
  "temp-artifacts"
do
  if ! "rm" \
    -f \
    -R \
    "/${directory:?}/"*
  then
    "error" "Failed to clear the \"${directory:?}\" directory!"
  fi
done

CURRENCIES_BUILD_REPORT="/temp-artifacts/currencies.build.log"
readonly CURRENCIES_BUILD_REPORT
export CURRENCIES_BUILD_REPORT

for tag in \
  "@agnostic" \
  "${dex_type:+"dex-${dex_type:?}"}"
do
  case "${tag?}" in
    ("") ;;
    (*)
      if ! "cargo" \
        -- \
        "each" \
        --tag "build" \
        --tag "${tag:?}" \
        "run" \
        --exact \
        -- \
        "build" \
        --profile "${mapped_build_profile:?}" \
        --lib \
        --locked \
        --target "wasm32-unknown-unknown" \
        --target-dir "/target/"
      then
        "error" "Failed to build contracts in workspace tagged with \"${tag}\"!"
      fi
  esac
done

output_directory="/target/wasm32-unknown-unknown/${mapped_build_profile:?}/"
readonly output_directory

if files="$(
  cd "${output_directory:?}" && \
    "find" \
      "." \
      "(" \
      "!" \
      -path "./*/*" \
      ")" \
      -type "f" \
      -name "*.wasm" \
      -print
)"
then
  if files="$(
    "sort" \
      <<EOF
${files:?}
EOF
  )"
  then
    readonly files
  else
    "error" "Failed to sort output directory files' paths via \"sort\"!"
  fi
else
  "error" "Failed to collect output directory files' paths!"
fi

if file_count="$(
  "wc" \
    -l \
    <<EOF
${files:?}
EOF
)"
then
  readonly file_count
else
  "error" "Failed to retrieve the output directories' binaries count via \"wc\"\
!"
fi

case "${file_count:?}" in
  ("0")
    "error" "Build produced no output! Expected ${contracts_count:?} contracts!"
    ;;
  ("${contracts_count:?}") ;;
  (*) "error" "Expected ${contracts_count:?} contracts, got ${file_count:?}!"
esac

while read -r wasm_path
do
  if ! wasm_name="$("basename" "${wasm_path:?}")"
  then
    "error" "Failed to extract basename from artifact file's path!"
  fi

  "echo" "Optimizing: ${wasm_name:?}"

  if "wasm-opt" \
    -Os \
    --inlining-optimizing \
    --signext-lowering \
    -o "/temp-artifacts/${wasm_name:?}" \
    "${output_directory}/${wasm_path:?}"
  then
    "echo" "Finished optimizing: ${wasm_name:?}"
  else
    "error" "Failed to run \"wasm-opt\" on \"${wasm_name:?}\"!"
  fi
done \
  <<EOF
${files:?}
EOF

if large_files="$(
  cd "/temp-artifacts/" && \
    "find" \
      "." \
      -type "f" \
      -name "*.wasm" \
      -size "+${max_binary_size:?}" \
      -printf "%f - %s bytes\n"
)"
then
  readonly large_files
else
  "error" "Failed to retrieve list of artifacts that are above allowed size!"
fi

case "${large_files?}" in
  ("") ;;
  (*) "error" "### These files are larger than the allowed limit, \
${max_binary_size:?}:
${large_files:?}"
esac

while read -r wasm_path
do
  (
    cd "/temp-artifacts/" && \
      "cosmwasm-check" \
        --available-capabilities "${cosmwasm_capabilities?}" \
        "./${wasm_path:?}"
  )
done \
  <<EOF
${files:?}
EOF

case "${dex_type?}" in
  ("")
    build_output_packages=""
    ;;
  (*) build_output_packages="currencies"
esac
readonly build_output_packages

case "${build_output_packages?}" in
  ("") ;;
  (*)
    "mkdir" "/artifacts/outputs/"

    while read -r build_output_package
    do
      if build_output="$(
        cd "${output_directory:?}" && \
          "find" \
            "." \
            -type "d" \
            -path "./build/${build_output_package:?}-????????????????/out"
      )"
      then
        case "${build_output?}" in
          ("")
            "error" "Retrieved list of build script output directories doesn't \
    contain \"${build_output_package:?}-<FINGERPRINT>\" directories!"
            ;;
          (*)
            if read -r build_output \
              <<EOF
${build_output:?}
EOF
            then
              "mkdir" "/artifacts/outputs/${build_output_package:?}/"

              "cp" \
                "${output_directory:?}/${build_output:?}/"* \
                "/artifacts/outputs/${build_output_package:?}/"
            else
              "error" "Failed to retrieve first line of build script output \
    directories!"
            fi
        esac
      else
        "error" "Failed to list build script binaries' output directory!"
      fi
    done \
      <<EOF
${build_output_packages?}
EOF
esac

if ! checksum="$(
  cd "/temp-artifacts/" && \
    "sha256sum" \
      -- \
      "./"*".wasm"
)"
then
  "error" "Failed to calculate artifact checksums!"
fi
readonly checksum

if ! "tee" \
  "/artifacts/checksums.txt" \
  <<EOF

Checksums:
${checksum:?}
EOF
then
  "error" "Failed to write checksums to artifacts directory!"
fi

"cp" \
  "/binaryen-version.txt" \
  "/release-version.txt" \
  "/rust-version.txt" \
  "/artifacts/"

temp_artifacts="$(
  "find" \
    "/temp-artifacts/" \
    "!" \
    "(" \
    -path "/temp-artifacts/" \
    -o \
    -path "/temp-artifacts/*/*" \
    ")"
)"

while read -r temp_artifact
do
  "mv" \
    "${temp_artifact:?}" \
    "/artifacts/"
done \
  <<EOF
${temp_artifacts?}
EOF
