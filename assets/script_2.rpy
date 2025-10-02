init:
    # Character pictures.
    image lada = Image("../characters/lada_vn.png")
    image alexei = Image("../characters/alexei_vn.png")

label start:

    music_generate "дождливый день в санкт петербурге"
    scene_generate "пасмурный день в санкт петербурге окраина города панельный дома нет людей вечер фон визуальной новелы"

    show lada

    lada "Hi, and welcome to the Ren'Py 4 demo program."
