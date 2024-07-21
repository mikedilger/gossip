Using the flatpak:
------------------

1) Install with:

      flatpak install --user gossip.0.11.1.flatpak

2) Run with:

      flatpak run com.mikedilger.gossip

3) If you get an eframe/glutin error, switch to the wgpu backend with this command, then
   afterwards you can run it normally as in step (2)

      flatpak run com.mikedilger.gossip wgpu_renderer true
