from setuptools import setup
from setuptools_rust import Binding, RustExtension

setup(
    name="libavicreator",
    version="0.1.0",
    rust_extensions=[
        RustExtension(
            "libavicreator",
            binding=Binding.PyO3,
            features=["python"],
        )
    ],
    packages=[],
    zip_safe=False,
)
