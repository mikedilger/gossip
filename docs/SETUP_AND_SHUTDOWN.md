# Setup and Shutdown

This may change, but this is approximately the order of events.

- bin::main()
    - setup logging
    - lib::init()
        - storage::init()
            - trigger database into existence
            - migrate
        - signer::init()
            - load from settings in storage
        - deletation init
        - setup wait-for-login state
    - setup async runtime
    - optionally handle command-line command and exit, else
    - spawn (two threads as below)

- spawn-thread
    - lib::run()
        - overlord::run()
            - maybe wait-for-login (the UI has to do it)
            - start fetcher
            - start People tasks
            - start relay picker
            - pick relays
            - subscribe discover
            - subscribe outbox
            - subscribe inbox
            - loop
                - Get and handle messages
                - or if shutdown variable is set, exit this loop
            - storage::sync()
            - set shutdown variable
            - message minions to shutdown
            - wait for minions to shutdown
    - end of spawn-thread

- main-thread
    - ui::run()
        - Setup and run the UI
        - if wait-for-login, prompt for password and login
        - once logged in, indicate such so the overlord can start, and run UI as normal
        - If shutdown variable is set, exit
    - Signal overlord to shutdown
    - Wait for spawn-thread to end
    - lib::shutdown()
        - storage::sync()
