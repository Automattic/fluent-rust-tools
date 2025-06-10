# frozen_string_literal: true

require 'tempfile'

RSpec.describe FluentTools::Converter do
  let(:converter) { described_class.new }

  describe '#fluent_to_android' do
    context "when input file doesn't exist" do
      it 'raises an error' do
        expect { converter.fluent_to_android('nonexistent.ftl', 'output.xml') }
          .to raise_error(FluentTools::Error, /Input file does not exist/)
      end
    end

    context 'when binary is not available' do
      it 'raises an error about missing binary' do
        Tempfile.create(['test', '.ftl']) do |input_file|
          allow(File).to receive(:executable?).and_return(false)
          expect { converter.fluent_to_android(input_file.path, 'output.xml') }
            .to raise_error(FluentTools::Error, /Binary not found or not executable/)
        end
      end
    end
  end

  describe '#android_to_fluent' do
    context "when input file doesn't exist" do
      it 'raises an error' do
        expect { converter.android_to_fluent('nonexistent.xml', 'output.ftl') }
          .to raise_error(FluentTools::Error, /Input file does not exist/)
      end
    end
  end
end
