# img2csv: Convert images of spreadsheets to CSV.

This is a sub-project of the [OpenPowerlifting](https://github.com/sstangl/openpowerlifting) project. A number of federations publish meet results as JPG screenshots of spreadsheets, and then refuse to send us the original spreadsheets.

Our options are either to enter in the meet results by hand or to attempt OCR. Existing OCR products do not work well. This is an attempt to write one that works in the limited domain we need it to.

The project is still in the beginning stages and is only occasionally attended to.

Currently, it can chop an input image into hundreds of smaller images, one per spreadsheet cell, but there is no good OCR engine to feed those images into. The next step is to train Tesseract on auto-generated testing data.

If you are interested in helping, please let me know.
