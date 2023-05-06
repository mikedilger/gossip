
INTEL AND APPLE SILICON
-----------------------

There are two releases, one for x86_64 (Intel) and one for arm64 (Apple silicon).
Choose the one that is right for your machine.


INSTALLING
----------

Double-click on the .dmg file, and then drag the Gossip icon into /Applications

Then eject the .dmg drive.


GETTING PAST THE SECURITY GUARD
-------------------------------

Because I am not an Apple Developer (I didn't like their contract), Apple's protection
mechanisms (Gatekeeper and/or XProtect) may give you errors like this:

    "Gossip" is damaged and can't be opened. You should move it to the Bin.

It is unlikely to be actually damaged. It is just that your computer cannot verify that
it is safe to run. If you trust me then you should do this:

1. Run 'sha256sum' on the .dmg file

   (If that command is not available run "brew install coreutils" first)

2. Compare the output to the published sum.  The sum will be published by the
   official gossip nostr account, as well as in a file on the github release.

   Official gossip nostr account: nprofile1qqsrjerj9rhamu30sjnuudk3zxeh3njl852mssqng7z4up9jfj8yupqprdmhxue69uhkummnw3ezumtfddjkg6tvvajhytnrdakj7ugumjz

   Github releases: https://github.com/mikedilger/gossip/releases

3. If they match, then the file is good. Tell your Mac that it is okay with this command:

   % sudo xattr -rd com.apple.quarantine /Applications/Gossip.app


VIEWING THE CONSOLE OUTPUT
--------------------------

Open up a console and run:

% /Applications/Gossip.app/Contents/MacOS/gossip-bin
