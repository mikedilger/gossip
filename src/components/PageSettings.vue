<script setup>
    import IconAt from './IconAt.vue'
    import IconBell from './IconBell.vue'
    import IconEncryptedMessage from './IconEncryptedMessage.vue'
    import IconFeed from './IconFeed.vue'
    import IconGear from './IconGear.vue'
    import IconInfo from './IconInfo.vue'
    import IconPerson from './IconPerson.vue'
    import IconPerson2 from './IconPerson2.vue'
    import IconQuote from './IconQuote.vue'
    import IconReply from './IconReply.vue'
    import IconRepost from './IconRepost.vue'
    import IconSpy from './IconSpy.vue'
    import IconVerified from './IconVerified.vue'
    import IconWalk from './IconWalk.vue'

    import Avatar from './Avatar.vue'
</script>

<template>
    <h2>settings</h2>
    <div class="main-scrollable">
        <p>
            Lorem ipsum dolor sit amet, consectetur adipiscing elit. Duis at neque vestibulum, imperdiet ipsum sed, accumsan mauris. Integer egestas urna quis leo bibendum egestas. Proin id lacus non dolor sagittis sodales. Curabitur et urna purus. Proin posuere purus vitae ipsum tristique faucibus. Proin semper id nibh ac interdum. Interdum et malesuada fames ac ante ipsum primis in faucibus. Curabitur tempus elit et venenatis mollis. Duis venenatis, urna vitae varius rutrum, sapien est fermentum justo, nec vulputate ex lectus vel arcu.
        </p>
        <Avatar url="https://avatars.githubusercontent.com/u/1669069?v=4"></Avatar>
        <br>
        <IconAt></IconAt>
        <IconBell></IconBell>
        <IconEncryptedMessage></IconEncryptedMessage>
        <IconFeed></IconFeed>
        <IconGear></IconGear>
        <IconInfo></IconInfo>
        <IconList></IconList>
        <IconPerson></IconPerson>
        <IconPerson2></IconPerson2>
        <IconQuote></IconQuote>
        <IconReply></IconReply>
        <IconRepost></IconRepost>
        <IconSpy></IconSpy>
        <IconVerified></IconVerified>
        <IconWalk></IconWalk>
        <p>
            Mauris fringilla, magna quis luctus laoreet, eros quam sollicitudin risus, sed porttitor augue dolor quis felis. Integer fringilla risus sed ex facilisis, nec posuere lacus egestas. Nulla aliquet luctus ornare. Donec vitae laoreet elit. Phasellus risus lacus, tempus quis mauris vestibulum, mattis consequat lectus. Morbi eu nulla ac sapien ullamcorper congue. Orci varius natoque penatibus et magnis dis parturient montes, nascetur ridiculus mus. Suspendisse eget tincidunt lorem. Nunc finibus nisl in varius eleifend. Etiam eleifend velit tempus ipsum placerat egestas. Vivamus fringilla nunc nibh, et placerat justo euismod sagittis.
        </p>
        <p>
            Curabitur aliquam, mauris et auctor tristique, ex massa gravida est, id imperdiet risus leo sit amet enim. Mauris eget tortor dignissim, interdum nisi eget, sodales ipsum. Maecenas ligula nisi, pulvinar sit amet cursus id, eleifend tristique lectus. Etiam pulvinar lectus sed mollis hendrerit. Nunc non auctor velit, a elementum felis. Aenean sollicitudin orci at luctus vestibulum. Duis pretium dolor non finibus rhoncus. Curabitur sit amet eros ut mauris consectetur finibus. Class aptent taciti sociosqu ad litora torquent per conubia nostra, per inceptos himenaeos. Pellentesque nec malesuada felis. Integer ut ligula vel felis commodo aliquet eget vel nunc. Vestibulum placerat nibh velit, feugiat tincidunt massa laoreet quis.
        </p>
        <p>
            Aenean placerat felis at eros scelerisque, ac commodo ex rhoncus. Sed et consequat ligula. Praesent nec egestas odio, id tempus ligula. Sed sodales nibh eu molestie egestas. Morbi tincidunt pharetra risus, quis aliquam felis dignissim sed. Phasellus ac erat erat. Nulla ipsum magna, elementum eget lectus non, fringilla feugiat lectus. Sed lobortis elit dui, quis feugiat nunc malesuada ut. Vivamus aliquet mollis metus. Vestibulum ante ipsum primis in faucibus orci luctus et ultrices posuere cubilia curae; Mauris rhoncus massa ut dolor vestibulum, ut porta mi placerat. Praesent interdum vel orci et ullamcorper. Ut dignissim erat arcu, in ornare tortor aliquam sollicitudin. Suspendisse potenti.
        </p>
        <p>
            Sed sed dolor non purus dapibus condimentum at id risus. Duis fermentum purus sit amet venenatis viverra. Maecenas finibus nulla et elit sollicitudin consequat. Nullam dapibus mollis purus, ac rutrum ex feugiat eget. Pellentesque vitae nisi condimentum mi imperdiet consequat. Maecenas laoreet ex ut laoreet pulvinar. Quisque non sem ante. Ut elementum lacinia lorem, eget convallis est maximus non. Etiam tincidunt hendrerit auctor. Pellentesque bibendum, diam eget lacinia dictum, nibh orci fringilla dolor, vitae sagittis nulla diam vel enim. Donec sit amet accumsan sem, vel convallis tortor. Curabitur et aliquam nibh. Donec quam tortor, interdum ut rhoncus et, aliquam eu ligula. In posuere vel felis sed ultrices.
        </p>
    </div>
</template>

<style scoped>
    div.main-scrollable {
        margin-top: 1em;
        padding-right: max(2em, 6vw);
        max-height: calc(100vh - 41px);
        overflow-y: scroll;
    }
</style>
