<template>
    <h2>yourself</h2>

    <hr>
    <h3>Generate a new Identity</h3>

    <div>
        <b>Weak Security</b> - Import your private Key
        <br>
        TBD.
        <ul>
            <li>By using this, your private key is likely displayed on the screen</li>
            <li>By using this, your private key probably remains in unallocated memory via the cut-n-paste buffer</li>
        </ul>
    </div>

    <div>
        <b>Medium Security</b> - Generate a private key
        <br>
        TBD.
        <ul>
            <li>You will need to provide a PIN to unlock your private key each time you use it, and we will promptly forget your private key and PIN after each event is signed.</li>
            <li>We will never display this private key on the screen.</li>
            <li>We will never write this private key to disk in unencrypted form.</li>
            <li>We will zero the memory that held your private key (and PIN) before freeing it</li>
        </ul>
    </div>

    <div>
        <b>Strong Security</b> - Use a physical hardware token
        <br>
        TBD.
        <ul>
            <li>You will need a compatible physical hardware token.</li>
            <li>You will need system libraries and configuration allowing us to access that token.</li>
        </ul>
    </div>

    <hr>

</template>
