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

SOFTWARE_RELEASE_ID="dev"
readonly SOFTWARE_RELEASE_ID
: "${SOFTWARE_RELEASE_ID:?}"
export SOFTWARE_RELEASE_ID

indentations="${indentations:-">"}"

run() {
  indentations="${indentations:?}>" \
    "${@:?}"
}

log() {
  set -eu

  case "${#:?}" in
    ("1")
      "echo" \
        "${indentations:?} ${1:?}" \
        >&2
      ;;
    (*)
      "echo" \
        "\"log\"/\"error\" function requires exactly one argument!" \
        >&2
  esac
}

error() {
  set -eu

  "log" "${@}"

  exit 1
}

case "${mode:?}" in
  ("runner")
    case "${#:?}" in
      ("0") ;;
      (*) "error" "The script doesn't take arguments!"
    esac
    ;;
  ("workspace"|"protocol"|"instance")
    workspace="${1:?"Workspace not specified!"}"
    readonly workspace
    : "${workspace:?}"
    shift

    case "${mode:?}" in
      ("workspace")
        case "${#:?}" in
          ("0") ;;
          (*) "error" "The script takes exactly one argument!"
        esac
        ;;
      (*)
        dex_type="${1:?"DEX type not specified!"}"
        readonly dex_type
        : "${dex_type:?}"
        shift

        case "${mode:?}" in
          ("instance")
            profile="${1:?"Profile not specified!"}"
            readonly profile
            : "${profile:?}"
            shift

            case "${#:?}" in
              ("0") ;;
              (*) "error" "The script takes exactly three arguments!"
            esac
            ;;
          (*)
            case "${#:?}" in
              ("0") ;;
              (*) "error" "The script takes exactly two arguments!"
            esac
        esac
    esac

    if ! cd "${workspace:?}"
    then
      "error" "Failed to change working directory to \"${workspace:?}\"!"
    fi
    ;;
  (*) "error" "Unknown environment mode!"
esac
