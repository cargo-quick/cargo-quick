#!/bin/bash

set -euxo pipefail

cargo install --locked --path $HOME/src/cargo-quick/cargo-quickbuild

# for crate in syn git-delta gitoxide probe-rs-cli pueue watchexec-cli xh ; do 
#     if ! [ -d  ~/tmp/$crate ] ; then
#         cargo clone $crate -- ~/tmp/$crate
#     fi
#     \in ~/tmp/$crate time cargo quickbuild build || echo "failed"
#     # \in ~/tmp/$crate cargo tree --edges=all --invert syn 
# done

cargo quickbuild repo find target/debug/.fingerprint/syn-0d23c7d14ae11633/lib-syn.json

# "target/debug/.fingerprint/syn-6ad1e5e6f3b856c8/dep-build-script-build-script-build" found in:
# "/Users/alsuren/tmp/quick/thiserror-impl-1.0.30-00da0095b58ac4ed117f2b68a426d4d6aa7b1ebe0d6fd775382130389cbce6f4.tar" with mtime 2022-09-09 14:01:52.828516870
# ```
# # thiserror-impl 1.0.30
# proc-macro2_1_0_36 = { package = "proc-macro2", version = "=1.0.36", features = ["default", "proc-macro"], default-features = false }
# quote_1_0_14 = { package = "quote", version = "=1.0.14", features = ["default", "proc-macro"], default-features = false }
# syn_1_0_94 = { package = "syn", version = "=1.0.94", features = ["clone-impls", "default", "derive", "extra-traits", "full", "parsing", "printing", "proc-macro", "quote"], default-features = false }
# thiserror-impl_1_0_30 = { package = "thiserror-impl", version = "=1.0.30", features = [], default-features = false }
# unicode-xid_0_2_1 = { package = "unicode-xid", version = "=0.2.1", features = ["default"], default-features = false }

# [build-dependencies]

# ```
# "target/debug/.fingerprint/syn-6ad1e5e6f3b856c8/dep-build-script-build-script-build" found in:
# "/Users/alsuren/tmp/quick/serde_derive-1.0.138-fc8d7d603e627d1a75f9f83a3b58422bc779d305ee094b7a818994933549ae36.tar" with mtime 2022-09-09 14:01:46.856682959
# ```
# # serde_derive 1.0.138
# proc-macro2_1_0_36 = { package = "proc-macro2", version = "=1.0.36", features = ["default", "proc-macro"], default-features = false }
# quote_1_0_14 = { package = "quote", version = "=1.0.14", features = ["default", "proc-macro"], default-features = false }
# serde_derive_1_0_138 = { package = "serde_derive", version = "=1.0.138", features = ["default"], default-features = false }
# syn_1_0_94 = { package = "syn", version = "=1.0.94", features = ["clone-impls", "default", "derive", "extra-traits", "full", "parsing", "printing", "proc-macro", "quote"], default-features = false }
# unicode-xid_0_2_1 = { package = "unicode-xid", version = "=0.2.1", features = ["default"], default-features = false }

# [build-dependencies]

# ```
(
    rm -rf ~/tmp/syn-minimal-repro
    cd ~/tmp
    cargo new syn-minimal-repro
    cd syn-minimal-repro
    # cargo add thiserror-impl@1.0.30 --no-default-features --features=default
    # cargo add serde_derive@1.0.138 --no-default-features
    echo 'serde_derive_1_0_138 = { package = "serde_derive", version = "=1.0.138", features = ["default"], default-features = false }' >> ~/tmp/syn-minimal-repro/Cargo.toml
    echo 'thiserror-impl_1_0_30 = { package = "thiserror-impl", version = "=1.0.30", features = [], default-features = false }' >> ~/tmp/syn-minimal-repro/Cargo.toml
    # With just these two lines, we get:
    #   serde_derive-1.0.138-bf94ca320b22ea189add388ee2e63e69cd479f6e93379b5e97e46470a04ced74 and thiserror-impl-1.0.30-26655618f122f007f14ab4323a55667ab56d60d860bf18010808033c5031f0cf
    # which compile and unpack just fine.
    # We want the ones that die in a fire, which are:
    #   serde_derive-1.0.138-fc8d7d603e627d1a75f9f83a3b58422bc779d305ee094b7a818994933549ae36 and thiserror-impl-1.0.30-00da0095b58ac4ed117f2b68a426d4d6aa7b1ebe0d6fd775382130389cbce6f4
    # The differences are that in the problematic packages:
    # * syn has "extra-traits", "full" as features
    # * a bunch of versions change
    # * unicode-ident is replaced with unicode-xid
    # * 
    echo '
syn_1_0_94 = { package = "syn", version = "=1.0.94", features = ["extra-traits"], default-features = false }
' >> ~/tmp/syn-minimal-repro/Cargo.toml
    \in ~/tmp/syn-minimal-repro/ cargo tree --edges=all

    for package in thiserror-impl serde_derive; do
        rm -rf ~/tmp/syn-minimal-repro/target/
        rm -rf ~/tmp/quick/$package*.tar
        # rm -f ~/tmp/$package.log

        # CARGO_LOG=cargo::core::compiler::fingerprint=trace \
        CARGO_LOG=cargo::core::compiler::unit_dependencies=warn,cargo::core::compiler::job_queue=warn,cargo=trace \
            cargo quickbuild build  || true # 2>&1 | sed 's/^[[]2022[^ ]*//' > ~/tmp/$package.log || true
    done

    sed -i '' 's/^[[]2022[^ ]*//' ~/tmp/quick/*.stderr
    code --diff ~/tmp/quick/syn*.stderr ~/tmp/quick/serde_derive*.stderr
)
