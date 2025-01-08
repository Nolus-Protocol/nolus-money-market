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

SOFTWARE_RELEASE_ID=dev
readonly SOFTWARE_RELEASE_ID
export SOFTWARE_RELEASE_ID

case "${#:?}" in
  ("0")
    repo_root="$(cd "../" && "pwd")"
    ;;
  (*)
    case "${1:?}" in
      ([!-]*)
        repo_root="$("pwd")"

        cd "./${1:?}"

        shift
        ;;
      (*)
        repo_root="$(cd "../" && "pwd")"
        ;;
    esac

    while :
    do
      case "${1:?}" in
        ("--help")
          "echo" "First argument of the script to be either a flag (start with \
leading '-') or the name of the workspace to run the linter in.
Without arguments and when the first argument is a flag, the current directory \
is assumed to the active workspace."
  
          exit "0"
          ;;
        ("-p"|"--package")
          package="${2:?}"
          readonly package

          shift 2
          ;;
        ("--")
          shift

          break
          ;;
        (*)
          "echo" \
            "" \
            >&2
          ;;
      esac

      case "${#:?}" in
        ("0")
          break
          ;;
      esac
    done
    ;;
esac
readonly repo_root
: "${repo_root:?}"

. "${repo_root:?}/scripts/check/configuration.sh"

run_base() {
  "cargo" \
      "each" \
      "run" \
      --external-command \
      --tag "ci" \
      "${@:?}"
}

run_with_package() {
  case "${package+"1"}" in
    ("1")
      "run_base" \
        --package "${package:?}" \
        "${@:?}"
      ;;
    (*)
      "run_base" "${@:?}"
      ;;
  esac
}

while read -r profile
do
  "run_with_package" \
    -- \
    "sh" \
    "${repo_root:?}/scripts/check/lint.sh" \
    --profile "${profile:?}" \
    "${@}"
done <<EOF
${profiles:?}
EOF
