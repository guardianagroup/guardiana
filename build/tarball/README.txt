GUARDIANA for Linux
===================

Guardiana turns this PC into the household's DNS guardian: it sees which
services each device tries to talk to, explains it in one sentence and cuts
only what you decide. Everything happens on this computer. No account, no
server, zero telemetry.

Install (.deb package, Debian and Ubuntu)
-----------------------------------------
    sudo apt install ./guardiana_*.deb

Install (tarball, any distribution with systemd)
------------------------------------------------
    tar xzf guardiana-*-linux-x86_64.tar.gz
    cd guardiana-*-linux-x86_64
    sudo ./instalar.sh

Installing does NOT change the system DNS. That is done from the panel, with
your permission, and can be undone in the same place.

After installing
----------------
    guardiana panel          opens the panel in the browser
    guardiana verify         checks the installation (hash, signature, ports)
    guardiana dns --status   says where the system DNS points
    guardiana hogar status   Home Mode status

Uninstall
---------
    sudo apt remove guardiana        (.deb package)
    sudo ./desinstalar.sh            (tarball)

Uninstalling puts the DNS back as it was and turns Home Mode off. The ledger
stays in /var/lib/guardiana; delete it with "sudo apt purge guardiana" or
"sudo ./desinstalar.sh --purge".

Before installing: check the hash
---------------------------------
    sha256sum guardiana_*.deb
Compare it with the one in SHA256SUMS and with the public ledger
(ledger.jsonl). How to do it, step by step: VERIFY.md.

What Guardiana does not do is in WHAT_IT_DOES_NOT_DO.md. Read it.
