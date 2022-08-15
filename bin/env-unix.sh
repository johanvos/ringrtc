#!/bin/sh

#
# Copyright 2019-2021 Signal Messenger, LLC
# SPDX-License-Identifier: AGPL-3.0-only
#

# Unix specific environment variables
# @note Nothing here yet.

prepare_workspace_platform() {
    echo "Preparing workspace for Unix..."
    $BIN_DIR/fetch-android-deps
    # @note Nothing here yet.
}
