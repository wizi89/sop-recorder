# Click position stays in desktop coordinates

A step's `click_x`/`click_y` are desktop coordinates, while its `marker_box` is in the saved image's
pixels. The server compares click positions of consecutive steps, and since each step is cropped to a
different window area, image coordinates would not be comparable between steps. A future reader
should not "fix" the click into image space to match the marker.
