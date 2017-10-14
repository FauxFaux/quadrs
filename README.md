```
% quadrs
usage: quadrs \
    from [-sr SAMPLE_RATE] [-format cf32|cs8|cu8|cs16] FILENAME.sr32k.cf32 \
   shift [-]FREQUENCY \
 lowpass [-power 20] [-decimate 8] FREQUENCY \
sparkfft [-width 128] [-stride STRIDE] [-range LOW:HIGH]
   write [-overwrite no] FILENAME_PREFIX \
     gen [-cos FREQUENCY]* SAMPLE_RATE \


Formats:

 * cf32: complex (little endian) floats, 32-bit (GNU-Radio, gqrx)
 *  cs8: complex      signed (integers),  8-bit (HackRF)
 *  cu8: complex    unsigned (integers),  8-bit (RTL-SDR)
 * cs16: complex      signed (integers), 16-bit (Fancy)
Error: no commands provided
```

## Worked example: FSK

We've got a file with some FSK info in it. It was sampled at
`21M` samples per second, and is stored in complex little-endian
float format.

```
$ quadrs \
    from fsk-example.sr21M.fc32 \
    sparkfft
```

![wide, usesless fft](screenshots/fsk-1.png)

The data we're after is the "noise" to the left of the horrible DC bias
line in the centre.

First, let's zoom in horizontally. This is done with a decimating low-pass
filter. Here, the big number is the allowed frequency band, and the `-decimate`
is the factor by which to reduce the width. Ignore the frequency band for now:

```
$ quadrs \
    from fsk-example.sr21M.fc32 \
    lowpass -decimate 16 2000000 \
    sparkfft
```

![narrow, wrong fft](screenshots/fsk-2.png)

Now we can see the FSK signal a bit! But we've ended up with random crap
on both sides, these are called "aliases", because our low-pass wasn't
aggressive enough. We'll fix that later:

First, let's centre it up:

```
$ quadrs \
    from fsk-example.sr21M.fc32 \
    shift 280000
    lowpass -decimate 16 2000000 \
    sparkfft
```

![centred, narrow](screenshots/fsk-3.png)

Notice how the shifting has ANGERED the DC bias bar into a wider bar.
We can punch our low-pass filter up a notch to shut it up again, by
increasing `power`, which is a work factor (bigger is better) and
narrowing the frequency:

```
$ quadrs \
    from fsk-example.sr21M.fc32 \
    shift 280000
    lowpass -power 200 -decimate 16 200000 \
    sparkfft
```

![centred, low-passed](screenshots/fsk-4.png)

Now the FFT is letting us down!

Let's ask it to stretch vertically (by lowering `stride`),
and use a smaller `width`, increasing the apparent "resolution" of
small flickers of signal. To compensate for the reduced width, we can
`decimate` harder. And make the font bigger.

```
$ quadrs \
    from fsk-example.sr21M.fc32 \
    shift 280000 \
    lowpass -power 200 -decimate 32 200000 \
    sparkfft -width 64 -stride 16
```

![smaller fft](screenshots/fsk-5.png)

Definitely looking like data now!